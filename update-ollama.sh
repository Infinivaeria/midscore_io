# Update Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Update every installed model
ollama list | tail -n +2 | awk '{print $1}' | while read -r model; do
    ollama pull "$model"
done

# Check everything
ollama --version
ollama list