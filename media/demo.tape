# VHS tape for llmtop demo GIF.
#
# Prerequisites (Linux/macOS):
#   brew install vhs ffmpeg ttyd            # macOS
#   sudo apt install ffmpeg && go install github.com/tsl0922/ttyd@latest
#   curl -fsSL https://raw.githubusercontent.com/charmbracelet/vhs/main/install.sh | bash
#
# Prerequisites (Windows):
#   - vhs already installed via `winget install charmbracelet.vhs`.
#   - ttyd does NOT run natively on Windows. Easiest path: run vhs in WSL2.
#     Inside WSL2:
#       sudo apt install ffmpeg
#       go install github.com/tsl0922/ttyd@latest
#       vhs media/demo.tape
#
# Run before recording:
#   ollama serve &                          # in separate terminal
#   ollama run qwen2.5-coder:7b "explain quicksort"   # produce real load
#   cargo install --path .                  # so `llmtop` resolves
#
# Render:
#   vhs media/demo.tape                     # outputs media/demo.gif

Output media/demo.gif

Set FontSize 14
Set Width 1100
Set Height 480
Set Theme "Catppuccin Mocha"
Set Padding 12
Set TypingSpeed 50ms

Type "llmtop"
Enter
Sleep 18s
Type "q"
