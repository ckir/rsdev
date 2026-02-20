Remove-Item "rsdev.txt" -ErrorAction SilentlyContinue
dir-to-text --use-gitignore -e "target" -e !pyTests -e "Cargo.lock" -e vendor -e .git .
