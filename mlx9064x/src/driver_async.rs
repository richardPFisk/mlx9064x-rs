// SPDX-License-Identifier: Apache-2.0
// Copyright © 2021 Will Ross

use embedded_hal_async::i2c;
use paste::paste;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::calculations::*;
use crate::common::*;
use crate::error::Error;
use crate::register::*;

/// A simple future that yields control back to the executor once.
struct YieldNow {
    yielded: bool,
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Yields control back to the async executor.
async fn yield_now() {
    YieldNow { yielded: false }.await
}

/// DRY macro for the set_* methods in `CameraDriverAsync` that modify a register field.
///
/// Most of the fields are boolean values, so that's the default type. Otherwise, add the type in
/// before the docstring.
macro_rules! set_register_field {
    { $register_access:ident, $field:ident, $doc:literal } => {
        set_register_field! {
            $register_access,
            $field,
            bool,
            $doc
        }
    };
    { $register_access:ident, $field:ident, $typ:ty, $doc:literal } => {
    paste! {
        #[doc = $doc]
        pub async fn [< set_ $field >](&mut self, new_value: $typ) -> Result<(), Error<I2C>> {
            let mut current = self.$register_access().await?;
            if current.$field() != new_value {
                current.[< set_ $field >](new_value);
                self.[< set_ $register_access >](current).await
            } else {
                Ok(())
            }
        }
    }};
}

/// The async camera driver for the MLX90640 and MLX90641 thermopiles.
///
/// These cameras offer higher resolutions and faster refresh rates than other common low-cost
/// thermal cameras, but they come with some differences in operation, as well as much more
/// processing required to end up with a grid of temperatures.
///
/// The biggest impact to users of these modules is that one of the  `generate_image_*` functions
/// will need to be called twice (once for each subpage) before a full image is available.

// HEIGHT and NUM_BYTES are const generics until generic_const_expr is stabilized (maybe). After
// that, point the associated constants on Clb::Camera (Clb::Camera::HEIGHT and {
// Clb::Camera::HEIGHT * Clb::Camera::WIDTH * 2 } ) can be used instead.
#[derive(Clone, Debug)]
pub struct CameraDriverAsync<Clb, I2C, const HEIGHT: usize, const NUM_BYTES: usize> {
    /// The I²C bus this camera is accessible on.
    bus: I2C,

    /// The I²C address this camera is accessible at.
    address: u8,

    /// The factory calibration data for a specific camera.
    calibration: Clb,

    /// Buffer for reading pixel data off of the camera.
    // I wish I could use const generics for computer parameters :/
    pixel_buffer: [u8; NUM_BYTES],

    /// ADC resolution correction factor.
    resolution_correction: f32,

    /// The most recent observed ambient temperature.
    ///
    /// The ambient temperature is calculated during image processing step. Save it for those
    /// applications that want the ambient temperature so a full recalculation isn't necessary.
    ambient_temperature: Option<f32>,

    /// The emissivity value to use when calculating pixel temperature.
    emissivity: f32,

    /// The current access pattern the camera is using.
    access_pattern: AccessPattern,

    /// The temperature of the ambient environment.
    ///
    /// This value is used as the "reflected temperature" for converting the observed IR data to
    /// actual temperatures.
    reflected_temperature: Option<f32>,
}

impl<'a, Clb, I2C, const HEIGHT: usize, const BUFFER_SIZE: usize>
    CameraDriverAsync<Clb, I2C, HEIGHT, BUFFER_SIZE>
where
    Clb: CalibrationData<'a>,
    I2C: i2c::I2c + i2c::ErrorType,
{
    /// Create a new `CameraDriverAsync`, obtaining the calibration data from the camera over I²C.
    pub async fn new(bus: I2C, address: u8) -> Result<Self, Error<I2C>>
    where
        Clb: FromI2CAsync<I2C, Ok = Clb, Error = Error<I2C>>,
    {
        let mut bus = bus;
        let calibration = Clb::from_i2c_async(&mut bus, address).await?;
        Self::new_with_calibration(bus, address, calibration).await
    }

    /// Create a `CameraDriverAsync` for accessing the camera at the given I²C address.
    ///
    /// MLX9064\*s can be configured to use any I²C address (except 0x00), but the default address
    /// is 0x33.
    pub async fn new_with_calibration(
        bus: I2C,
        address: u8,
        calibration: Clb,
    ) -> Result<Self, Error<I2C>> {
        // We own the bus now, make it mutable.
        let mut bus = bus;
        // Grab the control register values first
        // Need to map from I2C::Error manually as it's an associated type without bounds, so we
        // can't implement From<I2C:Error>
        let control = ControlRegister::from_i2c_async(&mut bus, address).await?;
        // Cache these values
        let resolution_correction =
            Clb::Camera::resolution_correction(calibration.resolution(), control.resolution());
        let access_pattern = control.access_pattern();
        // Choose an emissivity value to start with.
        let emissivity = calibration.emissivity().unwrap_or(1f32);
        Ok(Self {
            bus,
            address,
            calibration,
            pixel_buffer: [0u8; BUFFER_SIZE],
            resolution_correction,
            ambient_temperature: None,
            emissivity,
            access_pattern,
            reflected_temperature: None,
        })
    }

    async fn status_register(&mut self) -> Result<StatusRegister, Error<I2C>> {
        let register = StatusRegister::from_i2c_async(&mut self.bus, self.address).await?;
        Ok(register)
    }

    async fn set_status_register(&mut self, register: StatusRegister) -> Result<(), Error<I2C>> {
        register.to_i2c_async(&mut self.bus, self.address).await
    }

    fn update_control_register(&mut self, register: &ControlRegister) {
        // Update the resolution as well
        let calibrated_resolution = self.calibration.resolution();
        self.resolution_correction =
            Clb::Camera::resolution_correction(calibrated_resolution, register.resolution());
        self.access_pattern = register.access_pattern();
    }

    async fn control_register(&mut self) -> Result<ControlRegister, Error<I2C>> {
        let register = ControlRegister::from_i2c_async(&mut self.bus, self.address).await?;
        // Update the resolution as well
        self.update_control_register(&register);
        Ok(register)
    }

    async fn set_control_register(&mut self, register: ControlRegister) -> Result<(), Error<I2C>> {
        self.update_control_register(&register);
        register.to_i2c_async(&mut self.bus, self.address).await?;
        Ok(())
    }

    /// Get the last measured subpage.
    pub async fn last_measured_subpage(&mut self) -> Result<Subpage, Error<I2C>> {
        Ok(self.status_register().await?.last_updated_subpage())
    }

    /// Check if there is new data available, and if so, which subpage.
    pub async fn data_available(&mut self) -> Result<Option<Subpage>, Error<I2C>> {
        let register = self.status_register().await?;
        Ok(if register.new_data() {
            Some(register.last_updated_subpage())
        } else {
            None
        })
    }

    /// Clear the data available flag, signaling to the camera that the controller is ready for
    /// more data.
    ///
    /// This flag can only be reset by the controller.
    pub async fn reset_data_available(&mut self) -> Result<(), Error<I2C>> {
        let mut current = self.status_register().await?;
        current.reset_new_data();
        self.set_status_register(current).await
    }

    /// Check if the overwrite enabled flag is set.
    ///
    /// This flag is only effective when `data_hold_enabled` is active.
    pub async fn overwrite_enabled(&mut self) -> Result<bool, Error<I2C>> {
        Ok(self.status_register().await?.overwrite_enabled())
    }

    set_register_field! {
        status_register,
        overwrite_enabled,
        "Enabled (or disable) overwriting of data in RAM with new data."
    }

    /// Check if the camera is using subpages.
    ///
    /// When disabled, only one page will be measured. The default is to use subpages.
    pub async fn subpages_enabled(&mut self) -> Result<bool, Error<I2C>> {
        Ok(self.control_register().await?.use_subpages())
    }

    set_register_field! {
        control_register,
        use_subpages,
        "Enabled (or disable) the use of subpages."
    }

    /// Check if the "Enable data hold" flag is set.
    ///
    /// When this flag (bit 2 on 0x800D) is set, data is not copied to RAM unless the
    /// `enable_overwrite` flag is set. The default is for this mode to be disabled.
    pub async fn data_hold_enabled(&mut self) -> Result<bool, Error<I2C>> {
        Ok(self.control_register().await?.data_hold())
    }

    set_register_field! {
        control_register,
        data_hold,
        "Enabled (or disable) data holding."
    }

    /// Check if the camera is in subpage repeat mode.
    ///
    /// This flag only has an effect if [subpages][CameraDriverAsync::subpages_enabled] is enabled. In
    /// subpage repeat mode, only the subpage set in `selected_subpage` will be measured and
    /// updated. When disabled, the active subpage will alternate between the two. The default is
    /// disabled.
    pub async fn subpage_repeat(&mut self) -> Result<bool, Error<I2C>> {
        Ok(self.control_register().await?.subpage_repeat())
    }

    set_register_field! {
        control_register,
        subpage_repeat,
        "Enabled (or disable) subpage repeat mode."
    }

    /// Get the currently selected subpage when [subpage repeat] is enabled.
    ///
    /// This setting only has an effect when `subpage_repeat` is enabled. The default value is
    /// `Subpage::Zero`.
    ///
    /// [subpage repeat]: CameraDriverAsync::subpage_repeat
    pub async fn selected_subpage(&mut self) -> Result<Subpage, Error<I2C>> {
        Ok(self.control_register().await?.subpage())
    }

    set_register_field! {
        control_register,
        subpage,
        Subpage,
        "Set the currently selected subpage when [subpage repeat][CameraDriverAsync::subpage_repeat] is enabled."
    }

    /// Read the frame rate from the camera.
    ///
    /// The default frame rate is [2 FPS][FrameRate::Two].
    pub async fn frame_rate(&mut self) -> Result<FrameRate, Error<I2C>> {
        Ok(self.control_register().await?.frame_rate())
    }

    set_register_field! {
        control_register,
        frame_rate,
        FrameRate,
        "Set camera's frame rate."
    }
    // TODO: Add a special-use function for setting the frame rate in EEPROM.

    /// Get the current resolution of the ADC in the camera.
    ///
    /// The default resolution is [18 bits][Resolution::Eighteen].
    pub async fn resolution(&mut self) -> Result<Resolution, Error<I2C>> {
        Ok(self.control_register().await?.resolution())
    }

    set_register_field! {
        control_register,
        resolution,
        Resolution,
        "Set ADC resolution within the camera."
    }

    /// Get the current access pattern used by the camera when updating subpages.
    ///
    /// The default for the MLX90640 is the chess patterm while the default for the MLX90641 is the
    /// interleaved pattern.
    pub async fn access_pattern(&mut self) -> Result<AccessPattern, Error<I2C>> {
        Ok(self.control_register().await?.access_pattern())
    }

    set_register_field! {
        control_register,
        access_pattern,
        AccessPattern,
        "Set the access pattern used by the camera."
    }

    /// Get the emissivity value that is being used for calculations currently.
    ///
    /// The default emissivity is 1, unless a camera has a different value stored in EEPROM, in
    /// which case that value is used. The default can also be
    /// [overridden][CameraDriverAsync::override_emissivity], but this change is not stored on the camera.
    pub fn effective_emissivity(&self) -> f32 {
        self.emissivity
    }

    /// Override the emissivity value used in temperature calculations.
    ///
    /// The default emissivity is 1, unless a camera has a different value stored in EEPROM, in
    /// which case that value is used. This method allows a new value to be used to compensate for
    /// emissivity.
    pub fn override_emissivity(&mut self, new_value: f32) {
        self.emissivity = new_value;
    }

    /// Use the default emissivity value.
    ///
    /// This is the opposite to `override_emissivity`, as it uses the default emissivity from
    /// either the camera (if the camera has a value set) or 1.
    pub fn use_default_emissivity(&mut self) {
        let default_emissivity = self.calibration.emissivity();
        self.emissivity = default_emissivity.unwrap_or(1f32);
    }

    /// Retrieve the current reflected temperature value.
    ///
    /// When the temperature of the ambient environment is not known, this function will return
    /// `None`. See [`set_reflected_temperature`][Self::set_reflected_temperature] for more
    /// information.
    pub fn reflected_temperature(&self) -> Option<f32> {
        self.reflected_temperature
    }

    /// Set the reflected temperature value.
    ///
    /// This value is used to compensate for infrared radiation not being emitted by an object
    /// itself, but being emitted by the ambient environment and reflected by an object being
    /// measured. This value is distinct from the one from [`ambient_temperature`], but if not
    /// explicitly known it can be estimated from that value.
    pub fn set_reflected_temperature(&mut self, new_value: Option<f32>) {
        self.reflected_temperature = new_value;
    }

    /// Get the most recent ambient temperature calculation.
    ///
    /// What the datasheets (and this crate) refer to as "ambient temperature" should be better
    /// understood as the ambient temperature of the camera itself, not of the area being imaged.
    /// These values will usually be different because the camera generates some heat itself, and
    /// the sensor used for this value is within the camera module. See
    /// [`MelexisCamera::SELF_HEATING`] for more details.
    ///
    /// This value is calculated as part of the overall image calculations. If that
    /// process hasn't been performed yet (by calling
    /// [`generate_image_if_ready`][Self::generate_image_if_ready] or similar), this method will
    /// return `None`.
    pub fn ambient_temperature(&self) -> Option<f32> {
        self.ambient_temperature
    }

    /// The height of the thermal image, in pixels.
    pub fn height(&self) -> usize {
        // const generics make this silly.
        Clb::Camera::HEIGHT
    }

    /// The width of the thermal image, in pixels.
    pub fn width(&self) -> usize {
        Clb::Camera::WIDTH
    }

    async fn read_ram(&mut self, subpage: Subpage) -> Result<RamData, Error<I2C>> {
        read_ram_async::<Clb::Camera, I2C, HEIGHT>(
            &mut self.bus,
            self.address,
            self.access_pattern,
            subpage,
            &mut self.pixel_buffer,
        ).await
    }

    pub async fn generate_raw_image_subpage_to(
        &'a mut self,
        subpage: Subpage,
        destination: &mut [f32],
    ) -> Result<(), Error<I2C>> {
        let ram = self.read_ram(subpage).await?;
        let mut valid_pixels =
            Clb::Camera::pixels_in_subpage(subpage, self.access_pattern).into_iter();
        let t_a = raw_pixels_to_ir_data(
            &self.calibration,
            self.emissivity,
            self.resolution_correction,
            &self.pixel_buffer,
            ram,
            subpage,
            self.access_pattern,
            &mut valid_pixels,
            destination,
        );
        self.ambient_temperature = Some(t_a);
        Ok(())
    }

    pub async fn generate_image_subpage_to(
        &'a mut self,
        subpage: Subpage,
        destination: &mut [f32],
    ) -> Result<(), Error<I2C>> {
        let ram = self.read_ram(subpage).await?;
        let mut valid_pixels =
            Clb::Camera::pixels_in_subpage(subpage, self.access_pattern).into_iter();
        let t_a = raw_pixels_to_temperatures(
            &self.calibration,
            self.emissivity,
            self.reflected_temperature,
            self.resolution_correction,
            &self.pixel_buffer,
            ram,
            subpage,
            self.access_pattern,
            &mut valid_pixels,
            destination,
        );
        self.ambient_temperature = Some(t_a);
        Ok(())
    }

    /// Generate a thermal "image" from the camera's current data.
    ///
    /// This function does *not* check if there is new data, it just copies the current frame of
    /// data.
    pub async fn generate_image_to<'b: 'a>(
        &'b mut self,
        destination: &mut [f32],
    ) -> Result<(), Error<I2C>> {
        let subpage = self.last_measured_subpage().await?;
        self.generate_image_subpage_to(subpage, destination).await
    }

    /// Generate a thermal "image" from the camera's current data, if there's new data.
    ///
    /// This function first checks to see if there is new data available, and if there is it copies
    /// that data into the provided buffer. It will then clear the data ready flag afterwards,
    /// signaliing to the camera that we are ready for more data. The `Ok` value is a boolean for
    /// whether or not data was ready and copied.
    pub async fn generate_image_if_ready(
        &'a mut self,
        destination: &mut [f32],
    ) -> Result<bool, Error<I2C>> {
        // Not going through the helper methods on self to avoid infecting them with 'a
        let address = self.address;
        let bus = &mut self.bus;
        let pixel_buffer = &mut self.pixel_buffer;
        let mut status_register = StatusRegister::from_i2c_async(bus, address).await?;
        if status_register.new_data() {
            let subpage = status_register.last_updated_subpage();
            let mut valid_pixels =
                Clb::Camera::pixels_in_subpage(subpage, self.access_pattern).into_iter();
            let ram = read_ram_async::<Clb::Camera, I2C, HEIGHT>(
                bus,
                address,
                self.access_pattern,
                subpage,
                pixel_buffer,
            ).await?;
            let ambient_temperature = raw_pixels_to_temperatures(
                &self.calibration,
                self.emissivity,
                self.reflected_temperature,
                self.resolution_correction,
                &self.pixel_buffer,
                ram,
                subpage,
                self.access_pattern,
                &mut valid_pixels,
                destination,
            );
            self.ambient_temperature = Some(ambient_temperature);
            status_register.reset_new_data();
            status_register.to_i2c_async(bus, address).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Synchronize with the camera's frame update timing
    ///
    /// This function ignores any new data, then forces a new measurement by the camera, only
    /// returning when that measurement is complete. This can be used to synchronize frame access
    /// from the controller to the update time of the camera.
    pub async fn synchronize(&mut self) -> Result<(), Error<I2C>> {
        let mut status_register = self.status_register().await?;
        status_register.reset_new_data();
        status_register.set_overwrite_enabled(true);
        status_register.set_start_measurement();
        status_register.to_i2c_async(&mut self.bus, self.address).await?;
        // Poll for new data (async version should yield instead of spinning)
        loop {
            status_register = StatusRegister::from_i2c_async(&mut self.bus, self.address).await?;
            if status_register.new_data() {
                break;
            }
            // Yield to the async runtime instead of spinning
            // This allows other tasks to run while we wait
            yield_now().await;
        }
        Ok(())
    }
}