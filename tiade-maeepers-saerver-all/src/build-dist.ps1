# Define constants
$SOURCE_FOLDER = "C:\Ruby34-x64"
$TARGET_DIR = "C:\BashAssetFinal-build"
$SINGLE_FILE = "C:\BashAssetFinal\target\release\THE-META_GAME-Magi-Tek_Tek-Magi-Engiane.exe"
$DEST_DIR = Join-Path $TARGET_DIR "The-Meta_Game.exe"

# Function to compress a folder into a zip file
function Compress-Folder {
  param (
    [string]$Source,
    [string]$Destination
  )
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [System.IO.Compression.ZipFile]::CreateFromDirectory($Source, $Destination)
}

# Function to decompress a zip file
function Decompress-Zip {
  param (
    [string]$ZipFilePath,
    [string]$Destination
  )
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [System.IO.Compression.ZipFile]::ExtractToDirectory($ZipFilePath, $Destination)
}

# Create target directory
Write-Host "Creating target directory $TARGET_DIR."
New-Item -ItemType Directory -Force -Path $TARGET_DIR | Out-Null

# Remove old zip file
$ZipFilePath = Join-Path $TARGET_DIR "rustby-vm.zip"
if (Test-Path $ZipFilePath) {
  Write-Host "Removing rustby-vm.zip."
  Remove-Item -Force $ZipFilePath
}


# Remove the rustby-vm directory if it exists
$RustbyVmPath = Join-Path $TARGET_DIR "rustby-vm"
if (Test-Path $RustbyVmPath) {
  Write-Host "Removing directory $RustbyVmPath recursively."
  Remove-Item -Recurse -Force $RustbyVmPath
}

# Compress the source folder if it exists
if (Test-Path $SOURCE_FOLDER) {
  Write-Host "Compressing folder $SOURCE_FOLDER to $ZipFilePath."
  Compress-Folder -Source $SOURCE_FOLDER -Destination $ZipFilePath

 } else {
  Write-Host "Folder $SOURCE_FOLDER does not exist."
}

Write-Host "Done."

# Copy the executable to the new directory
if (Test-Path $DEST_DIR) {
  Write-Host "Removing old file $DEST_DIR."
  Remove-Item -Force $DEST_DIR
}

if (Test-Path $SINGLE_FILE) {
  Write-Host "Copying file $SINGLE_FILE to $DEST_DIR."
  Copy-Item -Path $SINGLE_FILE -Destination $DEST_DIR
} else {
  Write-Host "File $SINGLE_FILE does not exist."
}

Write-Host "Single file copied to new directory."


