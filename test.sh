#!/bin/bash

# Test script for R-Data Agent

echo "Building R-Data Agent..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✓ Build successful"
    echo ""
    echo "To run the application:"
    echo "  cargo run --release"
    echo ""
    echo "Sample data file created: sample_data.csv"
    echo ""
    echo "Quick test workflow:"
    echo "  1. Run: cargo run --release"
    echo "  2. Press 'l' to load file"
    echo "  3. Enter: sample_data.csv"
    echo "  4. Press Tab to go to Analysis tab"
    echo "  5. Press 's' for summary statistics"
    echo "  6. Press Tab to go to Visualizations tab"
    echo "  7. Press Space to view charts"
else
    echo "✗ Build failed"
    exit 1
fi
