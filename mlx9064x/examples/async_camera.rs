// SPDX-License-Identifier: Apache-2.0
// Example demonstrating async usage of MLX90640 thermal camera

use std::time::Duration;
use mlx9064x::{Mlx90640DriverAsync, Error};

// Mock async I2C implementation for demonstration
use embedded_hal_async::i2c::{ErrorType, I2c};

#[derive(Debug)]
pub struct MockI2cError;

impl embedded_hal::i2c::Error for MockI2cError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        embedded_hal::i2c::ErrorKind::Other
    }
}

impl std::fmt::Display for MockI2cError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Mock I2C error")
    }
}

impl std::error::Error for MockI2cError {}

pub struct MockAsyncI2c {
    data: std::collections::HashMap<(u8, u16), Vec<u8>>,
}

impl MockAsyncI2c {
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }

    pub fn set_mock_data(&mut self, device_addr: u8, reg_addr: u16, data: Vec<u8>) {
        self.data.insert((device_addr, reg_addr), data);
    }
}

impl ErrorType for MockAsyncI2c {
    type Error = MockI2cError;
}

impl I2c for MockAsyncI2c {
    async fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        // Mock implementation
        buffer.fill(0);
        Ok(())
    }

    async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        // Mock implementation
        Ok(())
    }

    async fn write_read(
        &mut self, 
        address: u8, 
        bytes: &[u8], 
        buffer: &mut [u8]
    ) -> Result<(), Self::Error> {
        // Mock implementation - return zeros for simplicity
        buffer.fill(0);
        Ok(())
    }

    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal_async::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        // Mock implementation
        for op in operations {
            match op {
                embedded_hal_async::i2c::Operation::Read(buffer) => buffer.fill(0),
                embedded_hal_async::i2c::Operation::Write(_) => {}
            }
        }
        Ok(())
    }
}

// Simple async runtime for this example
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MLX90640 Async Driver Example");
    
    // Create mock I2C bus
    let i2c_bus = MockAsyncI2c::new();
    
    // Default address for MLX90640
    let address = 0x33;
    
    // Note: In a real scenario, creating the driver would read calibration data
    // For this mock example, this would fail, so we'll just show the interface
    println!("Creating async MLX90640 driver...");
    
    match create_camera(i2c_bus, address).await {
        Ok(mut camera) => {
            // Demonstrate the async API
            demonstrate_async_operations(&mut camera).await?;
        }
        Err(e) => {
            println!("Note: Mock I2C doesn't have real calibration data");
            println!("Error (expected): {:?}", e);
            println!("In a real application, this would succeed with a real MLX90640 camera");
        }
    }
    
    Ok(())
}

async fn create_camera(
    i2c_bus: MockAsyncI2c, 
    address: u8
) -> Result<Mlx90640DriverAsync<MockAsyncI2c>, Error<MockAsyncI2c>> {
    // This demonstrates the async creation API
    Mlx90640DriverAsync::new(i2c_bus, address).await
}

async fn demonstrate_async_operations(
    camera: &mut Mlx90640DriverAsync<MockAsyncI2c>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Camera dimensions: {}x{}", camera.width(), camera.height());
    
    // Demonstrate async operations
    println!("Reading frame rate...");
    let frame_rate = camera.frame_rate().await?;
    println!("Current frame rate: {:?}", frame_rate);
    
    println!("Checking for new data...");
    let data_available = camera.data_available().await?;
    println!("Data available: {:?}", data_available);
    
    // Create buffer for temperature data
    let mut temperatures = vec![0f32; camera.height() * camera.width()];
    
    println!("Attempting to generate image...");
    let image_ready = camera.generate_image_if_ready(&mut temperatures).await?;
    println!("Image was ready: {}", image_ready);
    
    if let Some(ambient_temp) = camera.ambient_temperature() {
        println!("Ambient temperature: {:.2}°C", ambient_temp);
    }
    
    // Demonstrate synchronization (non-blocking version)
    println!("Synchronizing with camera...");
    camera.synchronize().await?;
    println!("Synchronization complete");
    
    // Show how you could periodically read frames
    println!("\nDemonstrating periodic frame reading...");
    for i in 0..3 {
        println!("Frame {} - checking for data...", i + 1);
        
        // In a real application, you might want to add a delay here
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let ready = camera.generate_image_if_ready(&mut temperatures).await?;
        if ready {
            println!("  - New temperature data captured!");
            // Process temperature data here...
        } else {
            println!("  - No new data available");
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_i2c() {
        let mut mock_i2c = MockAsyncI2c::new();
        let mut buffer = [0u8; 4];
        
        // Test that our mock I2C implementation works
        assert!(mock_i2c.write_read(0x33, &[0x80, 0x00], &mut buffer).await.is_ok());
        assert_eq!(buffer, [0, 0, 0, 0]);
    }
}