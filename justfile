set dotenv-load

default:
  just --list

run:
  tailwindcss -i ./style/input.css -o ./style/output.css --watch & cargo leptos watch

tailwind:
  tailwindcss -i ./style/input.css -o ./style/output.css --watch 

_remote-build:
  LEPTOS_BIN_TARGET_TRIPLE=$DEPLOY_TARGET_TRIPLE cargo leptos build --release

_remote-stop:
  ssh -t $DEPLOY_HOST "sudo systemctl --no-block stop $SERVICE_NAME"

_remote-start:
  ssh -t $DEPLOY_HOST "sudo service $SERVICE_NAME start"

_send-bin:
  scp target/$DEPLOY_TARGET_TRIPLE/release/jkcoxson $DEPLOY_HOST:$DEPLOY_PATH

rsync-site:
  rsync -r target/site $DEPLOY_HOST:$DEPLOY_PATH

deploy: _remote-stop _remote-build _send-bin rsync-site _remote-start

