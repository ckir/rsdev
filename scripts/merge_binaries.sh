#!/bin/bash

# Configuration
SEARCH_PATTERN="$HOME/Downloads/binaries-*.zip"
OUTPUT_ZIP="rsdev.zip"
REQUIRED_COUNT=5

# Expand the pattern into an array
FILES=($SEARCH_PATTERN)

# 1. Validation: Check if exactly 5 files exist
if [ ${#FILES[@]} -ne $REQUIRED_COUNT ]; then
    echo "Error: Expected $REQUIRED_COUNT files matching $SEARCH_PATTERN, but found ${#FILES[@]}."
    echo "Files found: ${FILES[@]}"
    exit 1
fi

# 2. Create a temporary workspace
TEMP_DIR=$(mktemp -d ./rsdev_merge_XXXXXX)

echo "Consolidating $REQUIRED_COUNT files..."

# 3. Extract each file into the workspace
for zip_file in "${FILES[@]}"; do
    echo "Extracting: $(basename "$zip_file")"
    unzip -q -o "$zip_file" -d "$TEMP_DIR"
done

# 4. Create the final archive
if [ "$(ls -A "$TEMP_DIR")" ]; then
    echo "Creating final archive: $OUTPUT_ZIP"
    
    # Zip the contents of the temp folder into the current directory
    (cd "$TEMP_DIR" && zip -r -q "../$OUTPUT_ZIP" .)
    
    echo "Success! Consolidated archive created at: $(pwd)/$OUTPUT_ZIP"
else
    echo "Extraction failed. No files found to compress."
fi

# 5. Cleanup
rm -rf "$TEMP_DIR"

