# Define constants
$SOURCE_FOLDER = "C:\Ruby34-x64"
$TARGET_DIR = "C:\BashAssetFinal-build"
$SINGLE_FILE = "C:\BashAssetFinal\target\release\THE-META_GAME-Magi-Tek_Tek-Magi-Engiane.exe"
$DEST_DIR = Join-Path $TARGET_DIR "The-Meta_Game.exe"



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


