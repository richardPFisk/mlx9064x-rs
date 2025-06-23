#!/bin/bash
PORT=$(ls /dev/cu.usbmodem* 2>/dev/null | head -1)
if [ -z "$PORT" ]; then
    echo "No USB device found at /dev/cu.usbmodem*"
    exit 1
fi
echo "Using port: $PORT"
espflash flash --monitor --chip esp32c6 --port "$PORT" "$@"