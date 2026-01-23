#!/bin/bash

# Configuration
FILE="rsdev.zip"
BUCKET_NAME="ckir-public"
S3_PATH="s3://$BUCKET_NAME/$FILE"

# 1. Check if the file exists locally
if [ ! -f "$FILE" ]; then
    echo "Error: $FILE not found. Please run the consolidation script first."
    exit 1
fi

echo "Uploading $FILE to $S3_PATH..."

# 2. Perform the upload
# -P or --acl-public: Makes the object publicly readable
# --guess-mime-type: Ensures the browser treats it as a zip file
s3cmd put "$FILE" "$S3_PATH" --acl-public --guess-mime-type

# 3. Verify success
if [ $? -eq 0 ]; then
    echo "------------------------------------------------"
    echo "Upload Successful!"
    echo "Public URL: https://$BUCKET_NAME.s3.amazonaws.com/$FILE"
else
    echo "Upload failed. Please check your s3cmd configuration."
    exit 1
fi

