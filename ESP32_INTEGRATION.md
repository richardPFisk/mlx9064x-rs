# ESP32 Integration Guide

This guide shows how to use the `mlx9064x` library with ESP32 microcontrollers.

## Hardware Setup

### Supported ESP32 Variants
- ESP32-C6 (tested)
- ESP32-C3 (should work)
- ESP32-S3 (should work)
- ESP32 (classic) (should work)

### MLX90640 Wiring

```
MLX90640 Breakout → ESP32-C6
VIN              → 3.3V
GND              → GND
SDA              → GPIO21 (with 4.7kΩ pull-up to 3.3V)
SCL              → GPIO22 (with 4.7kΩ pull-up to 3.3V)
```

**Important**: Always use pull-up resistors on I2C lines, even if your breakout board claims to include them.

### Alternative GPIO Pins
If GPIO21/22 are not available, try these combinations:
- GPIO2 (SDA) / GPIO3 (SCL)
- GPIO4 (SDA) / GPIO5 (SCL)  
- GPIO18 (SDA) / GPIO19 (SCL)

## Software Setup

### 1. Add to Your Cargo.toml

#### Using Private Git Repository:
```toml
[dependencies]
mlx9064x = { git = "https://github.com/yourusername/mlx9064x-rs", default-features = false, features = ["libm"] }
```

#### Using Local Path:
```toml
[dependencies]
mlx9064x = { path = "../mlx9064x-rs/mlx9064x", default-features = false, features = ["libm"] }
```

### 2. Required Features
- Use `default-features = false` for `no_std` environments
- Add `features = ["libm"]` for floating-point math support

### 3. ESP-specific Dependencies
```toml
esp-hal = { version = "=1.0.0-beta.1", features = ["esp32c6", "unstable"] }
esp-hal-embassy = { version = "0.8.1", features = ["esp32c6"] }
embassy-executor = { version = "0.7.0", features = ["task-arena-size-20480"] }
embassy-time = "0.4.0"
esp-alloc = "0.8.0"
esp-println = { version = "0.14.0", features = ["esp32c6"] }
```

## Basic Usage Example

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{clock::CpuClock, i2c::master::{Config, I2c}, timer::systimer::SystemTimer};
use esp_println::println;
use mlx9064x::Mlx90640Driver;

extern crate alloc;

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 64 * 1024);
    
    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    // Initialize I2C
    let i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22);

    // Initialize thermal camera
    let mut camera = Mlx90640Driver::new(i2c, 0x33).unwrap();
    let mut temperatures = alloc::vec![0f32; 24 * 32];
    
    loop {
        if camera.generate_image_if_ready(&mut temperatures).unwrap_or(false) {
            let avg: f32 = temperatures.iter().sum::<f32>() / temperatures.len() as f32;
            println!("Average temperature: {:.1}°C", avg);
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
```

## Troubleshooting

### No Device Found
1. Check wiring connections
2. Verify pull-up resistors (4.7kΩ)
3. Try different GPIO pins
4. Use I2C scanner to detect device

### Initialization Fails
1. Ensure stable 3.3V power supply
2. Add delays after power-on
3. Try slower I2C speeds
4. Check for I2C address conflicts

### Reading Errors
1. Verify heap allocation is sufficient (64KB recommended)
2. Check for timing issues
3. Monitor power supply stability

## I2C Scanner

Use this code to scan for I2C devices:

```rust
// Scan for I2C devices
for addr in 0x08..=0x77 {
    let mut buffer = [0u8; 1];
    if i2c.read(addr, &mut buffer).is_ok() {
        println!("Found device at 0x{:02X}", addr);
    }
}
```

MLX90640 should appear at address `0x33` (or sometimes `0x34`).

## Building and Flashing

```bash
# Build for ESP32-C6
cargo build --release

# Flash with espflash
espflash flash --monitor target/riscv32imac-unknown-none-elf/release/your-binary
```

## Performance Notes

- Frame rate: ~2-4 Hz typical
- Memory usage: ~64KB heap recommended
- I2C speed: 100kHz works reliably, 400kHz may work
- Processing time: ~50-100ms per frame