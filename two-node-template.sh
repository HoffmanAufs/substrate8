
echo "Clean tmp"
./target/debug/node-template purge-chain --base-path ./tmp/alice --chain local -y
./target/debug/node-template purge-chain --base-path ./tmp/bob --chain local -y
# ./target/debug/node-template purge-chain --base-path ./tmp/charlie --chain local -y
# ./target/debug/node-template purge-chain --base-path ./tmp/dave --chain local -y
# ./target/debug/node-template purge-chain --base-path ./tmp/eve --chain local -y

echo "Start alice (bootnode)"
deepin-terminal -e "./target/debug/node-template \
  --base-path ./tmp/alice \
  --chain local \
  --alice \
  --port 30333 \
  --ws-port 9945 \
  --rpc-port 9933 \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
  --validator" &
sleep 2s

# ./target/debug/node-template purge-chain --base-path ./tmp/alice --chain local -y;./target/debug/node-template --base-path ./tmp/alice --chain local --alice --port 30333 --ws-port 9945 --rpc-port 9933 --node-key 0000000000000000000000000000000000000000000000000000000000000001 --validator
# ./target/debug/node-template purge-chain --base-path ./tmp/charlie --chain local -y;./target/debug/node-template --base-path ./tmp/charlie --chain local --charlie --port 30335 --ws-port 9947 --rpc-port 9935 --validator --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp 
# ./target/debug/node-template purge-chain --base-path ./tmp/dave --chain local -y;./target/debug/node-template --base-path ./tmp/dave --chain local --dave --port 30336 --ws-port 9948 --rpc-port 9936 --validator --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp 

sleep 1s
echo "Start bob"
deepin-terminal -e "./target/debug/node-template \
  --base-path ./tmp/bob \
  --chain local \
  --bob \
  --port 30334 \
  --ws-port 9946 \
  --rpc-port 9934 \
  --validator \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# sleep 1s
# echo "Start charlie"
# deepin-terminal -e "./target/debug/node-template \
#   --base-path ./tmp/charlie\
#   --chain local \
#   --charlie \
#   --port 30335 \
#   --ws-port 9947 \
#   --rpc-port 9935 \
#   --validator \
#   --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"

# sleep 1s
# echo "Start dave"
# deepin-terminal -e "./target/debug/node-template \
#   --base-path ./tmp/dave \
#   --chain local \
#   --dave \
#   --port 30336 \
#   --ws-port 9948 \
#   --rpc-port 9936 \
#   --validator \
#   --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"