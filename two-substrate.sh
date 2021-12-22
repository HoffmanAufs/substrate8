
echo "Clean tmp"
./target/debug/substrate purge-chain --base-path ./tmp/alice --chain local -y
./target/debug/substrate purge-chain --base-path ./tmp/bob --chain local -y
./target/debug/substrate purge-chain --base-path ./tmp/charlie --chain local -y
# ./target/debug/substrate purge-chain --base-path ./tmp/dave --chain local -y
# ./target/debug/substrate purge-chain --base-path ./tmp/eve --chain local -y

echo "Start alice (bootnode)"
deepin-terminal -e "./target/debug/substrate \
  --base-path ./tmp/alice \
  --chain local \
  --alice \
  --port 30333 \
  --ws-port 9945 \
  --rpc-port 9933 \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator"
sleep 3s

echo "Start bob"
deepin-terminal -e "./target/debug/substrate \
  --base-path ./tmp/bob \
  --chain local \
  --bob \
  --port 30334 \
  --ws-port 9946 \
  --rpc-port 9934 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

sleep 1s
echo "Start charlie"
deepin-terminal -e "./target/debug/substrate \
  --base-path ./tmp/charlie\
  --chain local \
  --charlie \
  --port 30335 \
  --ws-port 9947 \
  --rpc-port 9935 \
  --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
  --validator \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# sleep 1s
# echo "Start dave"
# deepin-terminal -e "./target/debug/substrate \
#   --base-path ./tmp/dave \
#   --chain local \
#   --dave \
#   --port 30336 \
#   --ws-port 9948 \
#   --rpc-port 9936 \
#   --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
#   --validator \
#   --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# sleep 1s
# echo "Start eve"
# deepin-terminal -e "./target/debug/substrate \
#   --base-path ./tmp/eve \
#   --chain local \
#   --eve \
#   --port 30337 \
#   --ws-port 9949 \
#   --rpc-port 9937 \
#   --telemetry-url 'wss://telemetry.polkadot.io/submit/ 0' \
#   --validator \
#   --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# sleep 10s
# echo "Alice Keystore"
# curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d '{"jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params":["comm", "//Alice", "0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]}'

# sleep 1s
# echo "Bob Keystore"
# curl http://localhost:9934 -H "Content-Type:application/json;charset=utf-8" -d '{"jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params":["comm", "//Bob", "0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"]}'

# sleep 1s
# echo "Charlie Keystore"
# curl http://localhost:9935 -H "Content-Type:application/json;charset=utf-8" -d '{"jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params":["comm", "//Charlie", "0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22"]}'

# sleep 1s
# echo "Dave Keystore"
# curl http://localhost:9936 -H "Content-Type:application/json;charset=utf-8" -d '{"jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params":["comm", "//Dave", "0x306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20"]}'

# sleep 1s
# echo "Eve Keystore"
# curl http://localhost:9937 -H "Content-Type:application/json;charset=utf-8" -d '{"jsonrpc":"2.0", "id":1, "method":"author_insertKey", "params":["comm", "//Eve", "0xe659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e"]}'