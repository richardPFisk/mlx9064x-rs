# Using mlx9064x Library in Your Project

This document explains how to use this mlx9064x library in your other Rust projects.

## Option 1: Private Git Repository (Recommended)

### Step 1: Push to Private GitHub Repository
```bash
cd /path/to/mlx9064x-rs
git add .
git commit -m "Add ESP32 support and examples"
git remote add origin https://github.com/yourusername/mlx9064x-rs.git
git push -u origin main
```

### Step 2: Use in Your Project
Add to your project's `Cargo.toml`:
```toml
[dependencies]
mlx9064x = { git = "https://github.com/yourusername/mlx9064x-rs", default-features = false, features = ["libm"] }
```

For ESP32 projects, also add:
```toml
esp-hal = { version = "=1.0.0-beta.1", features = ["esp32c6", "unstable"] }
embassy-executor = { version = "0.7.0", features = ["task-arena-size-20480"] }
embassy-time = "0.4.0"
esp-alloc = "0.8.0"
```

## Option 2: Local Path Dependency

Keep the mlx9064x library locally and reference it by path:

```toml
[dependencies]
mlx9064x = { path = "../mlx9064x-rs/mlx9064x", default-features = false, features = ["libm"] }
```

## Option 3: Specific Git Branch/Tag

You can pin to a specific branch or tag:
```toml
[dependencies]
# Use specific branch
mlx9064x = { git = "https://github.com/yourusername/mlx9064x-rs", branch = "esp32-support" }

# Use specific commit
mlx9064x = { git = "https://github.com/yourusername/mlx9064x-rs", rev = "abc123" }

# Use tag
mlx9064x = { git = "https://github.com/yourusername/mlx9064x-rs", tag = "v0.3.1" }
```

## Basic Usage in Your Project

```rust
use mlx9064x::Mlx90640Driver;
use esp_hal::i2c::master::{Config, I2c};

// Initialize I2C
let i2c = I2c::new(peripherals.I2C0, Config::default())
    .unwrap()
    .with_sda(peripherals.GPIO21)
    .with_scl(peripherals.GPIO22);

// Initialize MLX90640
let mut thermal_camera = Mlx90640Driver::new(i2c, 0x33)?;

// Read thermal data
let mut temperatures = vec![0f32; 24 * 32];
if thermal_camera.generate_image_if_ready(&mut temperatures)? {
    // Process temperature data
    for temp in &temperatures {
        println!("Pixel: {:.1}°C", temp);
    }
}
```

## Features to Enable

### For ESP32/embedded use:
```toml
mlx9064x = { git = "...", default-features = false, features = ["libm"] }
```

### For desktop development:
```toml
mlx9064x = { git = "...", features = ["std"] }
```

### For async applications:
```toml
mlx9064x = { git = "...", features = ["std", "async-examples"] }
```

## Build Configuration

For ESP32 projects, ensure your `.cargo/config.toml` includes:
```toml
[target.riscv32imac-unknown-none-elf]
rustflags = ["-C", "link-arg=-Tlinkall.x"]

[build]
target = "riscv32imac-unknown-none-elf"

[unstable]
build-std = ["core", "alloc"]
```

## Repository Structure

When you publish your repository, include:
```
mlx9064x-rs/
├── mlx9064x/               # Main library
│   ├── src/
│   ├── examples/
│   └── Cargo.toml
├── ESP32_INTEGRATION.md    # ESP32 setup guide
├── USAGE.md               # This file
└── README.md              # Main documentation
```

## Updating the Library

To update to a newer version:
```bash
# For git dependencies
cargo update -p mlx9064x

# Or update your Cargo.toml to point to a newer commit/tag
```

## Contributing Back

If you make improvements:
1. Consider contributing back to the original repository
2. Create pull requests for useful features
3. Document ESP32-specific functionality

## License

This library is Apache-2.0 licensed. Ensure your project is compatible.