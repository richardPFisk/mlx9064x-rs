# MLX9064x Async Driver Usage

This crate now provides async-first support for MLX90640 and MLX90641 thermal cameras using embedded-hal 1.0.0 and embedded-hal-async 1.0.0.

## Features

- **Non-blocking operations**: All I2C operations are async and yield control back to the executor
- **Compatible with embedded-hal 1.0.0**: Uses the latest embedded-hal traits
- **Async-first API**: Designed for modern embedded async runtimes
- **Both camera models**: Supports MLX90640 and MLX90641

## Basic Usage

```rust
use mlx9064x::{Mlx90640DriverAsync, Error};
use embedded_hal_async::i2c::I2c;

async fn example_usage<I2C>(i2c_bus: I2C) -> Result<(), Error<I2C>>
where
    I2C: I2c + embedded_hal_async::i2c::ErrorType,
{
    // Create the async driver
    let mut camera = Mlx90640DriverAsync::new(i2c_bus, 0x33).await?;
    
    // Get camera dimensions
    let width = camera.width();
    let height = camera.height();
    println!("Camera: {}x{}", width, height);
    
    // Configure frame rate
    let frame_rate = camera.frame_rate().await?;
    println!("Current frame rate: {:?}", frame_rate);
    
    // Create buffer for temperature data
    let mut temperatures = vec![0f32; height * width];
    
    // Non-blocking frame reading
    loop {
        // Check for new data without blocking
        if camera.generate_image_if_ready(&mut temperatures).await? {
            println!("New temperature data available!");
            
            // Process temperature data here...
            if let Some(ambient) = camera.ambient_temperature() {
                println!("Ambient temperature: {:.2}°C", ambient);
            }
        }
        
        // Yield control to other tasks
        // (In real applications, you might want to add a delay here)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
```

## Key Async Methods

All the main camera operations are now async:

- `new(bus, address)` - Create driver and read calibration data
- `frame_rate()` - Get current frame rate setting
- `set_frame_rate(rate)` - Set new frame rate
- `data_available()` - Check if new data is ready
- `generate_image_if_ready(buffer)` - Non-blocking image capture
- `synchronize()` - Sync with camera timing (yields instead of spinning)

## Non-blocking Synchronization

The original blocking `synchronize()` method used a busy-wait loop. The async version yields control:

```rust
// OLD (blocking):
// while !status_register.new_data() {
//     status_register = StatusRegister::from_i2c(&mut self.bus, self.address)?;
//     core::hint::spin_loop();  // Blocks the thread
// }

// NEW (async):
loop {
    status_register = StatusRegister::from_i2c_async(&mut self.bus, self.address).await?;
    if status_register.new_data() {
        break;
    }
    yield_now().await;  // Yields to other tasks
}
```

## Available Types

- `Mlx90640DriverAsync<I2C>` - Async MLX90640 driver
- `Mlx90641DriverAsync<I2C>` - Async MLX90641 driver
- `CameraDriverAsync<Clb, I2C, HEIGHT, NUM_BYTES>` - Generic async driver

## Compatibility

- **embedded-hal**: 1.0.0
- **embedded-hal-async**: 1.0.0
- **no_std**: Compatible
- **Async runtimes**: Works with any async runtime (tokio, embassy, etc.)

## Example

See `examples/async_camera.rs` for a complete working example with a mock I2C implementation.

```bash
cargo run --example async_camera --features async-examples
```