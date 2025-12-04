# Celestia Adapter Evaluator - Cheat Sheet

## Run Command (with logging)

```bash
RUST_LOG=info cargo run --release -- \
  --namespace "7daysofsov" \
  --rpc-endpoint "http://YOUR_NODE:26658" \
  --rpc-token "YOUR_RPC_TOKEN" \
  --grpc-endpoint "http://YOUR_NODE:9090" \
  --signer-private-key "YOUR_HEX_PRIVATE_KEY" \
  --run-for-seconds 604800 2>&1 | tee /root/celestia-test.log
```



## tmux Commands

| Action | Command |
|--------|---------|
| New session | `tmux new -s celestia-test` |
| Attach | `tmux a -t celestia-test` |
| List sessions | `tmux ls` |
| Kill session | `tmux kill-session -t celestia-test` |

### tmux Shortcuts (inside session)

| Action | Shortcut |
|--------|----------|
| Detach | `Ctrl+B` then `D` |
| Scroll mode | `Ctrl+B` then `[` |
| Exit scroll | `q` |

## Memory Monitoring

```bash
# Watch memory usage (updates every 60s)
watch -n 60 'free -h && echo "---" && ps aux --sort=-%mem | head -5'

# One-time check
free -h

# Check specific process
ps aux | grep celestia

# Detailed memory info
cat /proc/meminfo | head -10
```

## Rebuild Binary

```bash
source ~/.cargo/env && cd /root/celestia-adapter-evaluator && cargo build --release
```

## Log Monitoring

```bash
# Follow log in real-time
tail -f /root/celestia-test.log

# Last 100 lines
tail -100 /root/celestia-test.log

# Search for errors
grep -i error /root/celestia-test.log
```
