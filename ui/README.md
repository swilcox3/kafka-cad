# Client UI

This is a simple Javascript client viewer that displays object updates on the screen.  It connects over Websocket to the Update Server to get and display the updates.  It doesn't currently connect to Operations Server, but there's no reason it couldn't.  I'm just focusing on the backend right now, not user interaction.

In order to use this, you'll need to run `npm install` in this directory, then install [protoc](https://github.com/protocolbuffers/protobuf/releases/tag/v3.11.4) and [grpc-web](https://github.com/grpc/grpc-web/releases) and install them somewhere in PATH.  You may need to rename the grpc-web executable to `protoc-gen-grpc-web`.

Run `npm run build`.  The UI is now compiled and will be served by ui-server.
