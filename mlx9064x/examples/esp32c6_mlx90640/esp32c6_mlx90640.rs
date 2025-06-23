#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Io;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::println;
use mlx9064x::{Mlx90640Driver, Mlx90641Driver, MelexisCamera};

extern crate alloc;
use alloc::vec;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    // Configure I2C pins - adjust these based on your wiring
    // Common ESP32-C6 I2C pins: SDA=GPIO6, SCL=GPIO7
    let sda = io.pins.gpio6;
    let scl = io.pins.gpio7;

    // Initialize I2C with 100kHz for MLX9064X
    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig {
            frequency: 100_000.Hz(),
            ..Default::default()
        },
    )
    .with_sda(sda)
    .with_scl(scl);

    println!("ESP32-C6 MLX9064X Test Starting...");

    // Try both common I2C addresses
    let addresses = [0x33, 0x34];
    let mut found_device = false;

    for &addr in &addresses {
        println!("Checking I2C address 0x{:02X}...", addr);
        
        // Try to detect if it's MLX90640 or MLX90641
        match detect_and_test_sensor(i2c, addr).await {
            Ok(sensor_type) => {
                println!("Found {} at address 0x{:02X}", sensor_type, addr);
                found_device = true;
                
                // Run continuous temperature monitoring
                match sensor_type {
                    SensorType::MLX90640 => {
                        run_mlx90640_loop(i2c, addr).await;
                    }
                    SensorType::MLX90641 => {
                        run_mlx90641_loop(i2c, addr).await;
                    }
                }
                break;
            }
            Err(e) => {
                println!("No device at 0x{:02X}: {:?}", addr, e);
            }
        }
    }

    if !found_device {
        println!("No MLX9064X sensor found!");
        println!("Please check:");
        println!("- Wiring: SDA=GPIO6, SCL=GPIO7");
        println!("- Power: 3.3V to sensor");
        println!("- Pull-up resistors on I2C lines (4.7k typical)");
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[derive(Debug)]
enum SensorType {
    MLX90640,
    MLX90641,
}

async fn detect_and_test_sensor<I2C>(
    mut i2c: I2C,
    address: u8,
) -> Result<SensorType, &'static str>
where
    I2C: embedded_hal::i2c::I2c,
{
    // Try to read the ID register (common to both sensors)
    let mut id_bytes = [0u8; 3];
    
    // MLX9064X ID register is at 0x240F
    let id_register = [0x24, 0x0F];
    
    if i2c.write_read(address, &id_register, &mut id_bytes).is_err() {
        return Err("No response from device");
    }

    // Try creating MLX90640 first (more common)
    match Mlx90640Driver::new(i2c, address) {
        Ok(_) => Ok(SensorType::MLX90640),
        Err(i2c_back) => {
            // If MLX90640 fails, try MLX90641
            match Mlx90641Driver::new(i2c_back, address) {
                Ok(_) => Ok(SensorType::MLX90641),
                Err(_) => Err("Failed to initialize as either MLX90640 or MLX90641"),
            }
        }
    }
}

async fn run_mlx90640_loop<I2C>(i2c: I2C, address: u8)
where
    I2C: embedded_hal::i2c::I2c,
{
    println!("Initializing MLX90640...");
    
    let mut camera = match Mlx90640Driver::new(i2c, address) {
        Ok(cam) => cam,
        Err(_) => {
            println!("Failed to create MLX90640 driver");
            return;
        }
    };

    println!("MLX90640 initialized successfully!");
    println!("Resolution: {}x{}", camera.width(), camera.height());

    // Create buffer for temperature data
    let mut temperatures = vec![0f32; camera.height() * camera.width()];

    // Main measurement loop
    let mut frame_count = 0;
    loop {
        // Check if new data is available
        match camera.data_available() {
            Ok(true) => {
                // Generate temperature image
                match camera.generate_image_if_ready(&mut temperatures) {
                    Ok(true) => {
                        frame_count += 1;
                        
                        // Get ambient temperature
                        if let Some(ambient) = camera.ambient_temperature() {
                            println!("\nFrame {}: Ambient temp: {:.1}°C", frame_count, ambient);
                        }

                        // Find min/max temperatures
                        let (min_temp, max_temp) = find_temp_range(&temperatures);
                        println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);

                        // Print center pixel temperature
                        let center_idx = (camera.height() / 2) * camera.width() + (camera.width() / 2);
                        println!("Center pixel: {:.1}°C", temperatures[center_idx]);

                        // Optional: Print simple ASCII visualization
                        print_thermal_image(&temperatures, camera.width(), camera.height());
                    }
                    Ok(false) => {
                        // No new frame ready yet
                    }
                    Err(e) => {
                        println!("Error generating image: {:?}", e);
                    }
                }
            }
            Ok(false) => {
                // No new data available
            }
            Err(e) => {
                println!("Error checking data availability: {:?}", e);
            }
        }

        // Delay between readings
        Timer::after(Duration::from_millis(100)).await;
    }
}

async fn run_mlx90641_loop<I2C>(i2c: I2C, address: u8)
where
    I2C: embedded_hal::i2c::I2c,
{
    println!("Initializing MLX90641...");
    
    let mut camera = match Mlx90641Driver::new(i2c, address) {
        Ok(cam) => cam,
        Err(_) => {
            println!("Failed to create MLX90641 driver");
            return;
        }
    };

    println!("MLX90641 initialized successfully!");
    println!("Resolution: {}x{}", camera.width(), camera.height());

    // Create buffer for temperature data
    let mut temperatures = vec![0f32; camera.height() * camera.width()];

    // Main measurement loop
    let mut frame_count = 0;
    loop {
        // Check if new data is available
        match camera.data_available() {
            Ok(true) => {
                // Generate temperature image
                match camera.generate_image_if_ready(&mut temperatures) {
                    Ok(true) => {
                        frame_count += 1;
                        
                        // Get ambient temperature
                        if let Some(ambient) = camera.ambient_temperature() {
                            println!("\nFrame {}: Ambient temp: {:.1}°C", frame_count, ambient);
                        }

                        // Find min/max temperatures
                        let (min_temp, max_temp) = find_temp_range(&temperatures);
                        println!("Temperature range: {:.1}°C to {:.1}°C", min_temp, max_temp);

                        // Print center pixel temperature
                        let center_idx = (camera.height() / 2) * camera.width() + (camera.width() / 2);
                        println!("Center pixel: {:.1}°C", temperatures[center_idx]);

                        // Optional: Print simple ASCII visualization
                        print_thermal_image(&temperatures, camera.width(), camera.height());
                    }
                    Ok(false) => {
                        // No new frame ready yet
                    }
                    Err(e) => {
                        println!("Error generating image: {:?}", e);
                    }
                }
            }
            Ok(false) => {
                // No new data available
            }
            Err(e) => {
                println!("Error checking data availability: {:?}", e);
            }
        }

        // Delay between readings
        Timer::after(Duration::from_millis(100)).await;
    }
}

fn find_temp_range(temps: &[f32]) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    
    for &temp in temps {
        if temp.is_finite() {
            if temp < min {
                min = temp;
            }
            if temp > max {
                max = temp;
            }
        }
    }
    
    (min, max)
}

fn print_thermal_image(temps: &[f32], width: usize, height: usize) {
    let (min_temp, max_temp) = find_temp_range(temps);
    let range = max_temp - min_temp;
    
    if range == 0.0 {
        return;
    }

    println!("\nThermal Image (ASCII):");
    
    // ASCII characters for different temperature levels
    let chars = [' ', '.', ':', '-', '=', '+', '*', '#', '@'];
    
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let temp = temps[idx];
            
            // Normalize temperature to 0-1 range
            let normalized = ((temp - min_temp) / range).clamp(0.0, 1.0);
            
            // Map to character index
            let char_idx = (normalized * (chars.len() - 1) as f32) as usize;
            print!("{}", chars[char_idx]);
        }
        println!();
    }
}