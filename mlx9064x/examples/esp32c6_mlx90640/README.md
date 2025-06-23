# ESP32-C6 MLX9064X Test Example

This example demonstrates how to use the MLX90640/MLX90641 thermal camera with an ESP32-C6 microcontroller.

## Hardware Setup

### Wiring
Connect your MLX9064X sensor to the ESP32-C6:

- **VDD** → 3.3V
- **GND** → GND
- **SDA** → GPIO6
- **SCL** → GPIO7

**Important**: Add 4.7kΩ pull-up resistors on both SDA and SCL lines to 3.3V.

### I2C Address
- MLX90640/MLX90641 typically use address 0x33
- Some modules may use 0x34
- The example will automatically detect both addresses

## Prerequisites

1. Install Rust ESP toolchain:
```bash
# Install espup
cargo install espup
espup install

# Source the environment
. ~/export-esp.sh
```

2. Install espflash for flashing:
```bash
cargo install espflash
```

## Building and Running

1. Navigate to the example directory:
```bash
cd mlx9064x/examples/esp32c6_mlx90640
```

2. Build the project:
```bash
cargo build --release
```

3. Flash to your ESP32-C6:
```bash
# Replace /dev/ttyUSB0 with your serial port
espflash flash --monitor target/riscv32imac-unknown-none-elf/release/esp32c6-mlx9064x-test --port /dev/ttyUSB0
```

For macOS, the port might be something like `/dev/cu.usbserial-*` or `/dev/tty.usbmodem*`.

## What the Example Does

1. **Initializes I2C** at 100kHz (standard speed for MLX9064X)
2. **Auto-detects** if you have MLX90640 or MLX90641
3. **Continuously reads** temperature data
4. **Displays**:
   - Ambient temperature
   - Min/max temperature in the frame
   - Center pixel temperature
   - ASCII thermal image visualization

## Troubleshooting

### No sensor detected
- Check wiring connections
- Verify pull-up resistors are installed
- Ensure sensor has 3.3V power
- Try the alternate I2C address

### I2C errors
- Reduce I2C speed if needed
- Check for loose connections
- Ensure adequate power supply

### Build errors
- Make sure you have the ESP toolchain installed
- Run `. ~/export-esp.sh` before building

## Customization

To change I2C pins, modify these lines in the code:
```rust
let sda = io.pins.gpio6;  // Change to your SDA pin
let scl = io.pins.gpio7;  // Change to your SCL pin
```

To adjust the frame rate or other camera settings, refer to the mlx9064x documentation.