# kafka-cad
![Build](https://github.com/swilcox3/kafka-cad/workflows/Build/badge.svg) ![Unit Tests](https://github.com/swilcox3/kafka-cad/workflows/Unit%20Tests/badge.svg)


This is a cloud-native architectural CAD prototype, with these design pillars:
1. The user will never lose work or corrupt data.
2. The UI will never lock up while waiting for results.
3. An arbitrary number of users can work in the same model simultaneously and/or asynchronously.
4. Good performance is maintained regardless of model size.

In order to support goals 1 and 3, the backend is designed like a version control system.  The full history of the model is tracked in an append-only commit log.  Eventually full git-like functionality will be implemented, with branches, merges, backouts, and reverts.  
In order to support goals 2 and 4, most processing takes place server-side.  The client is only responsible for user interaction and rendering, and makes full use of async code to remain responsive while waiting for the backend.  This also allows multiple client frontends on different platforms to be supported eventually, including mobile.  The client only receives the bare minimum amount of data to display the model.  Eventually the model will also be partitioned/streamed as the user navigates within it to maintain client-side performance.  Server-side, operations must be specifically designed to avoid model-wide operations as much as possible.

# Kafka Topics
1. ObjectState - This is the full history of all changes to all objects in the model.
2. ClientRepresentations - Parallels ObjectState with the client representations of all objects, which includes tessellations, drawing views, etc.

# Backend Design
This prototype is mostly focused on the backend.  The commit log takes place in Kafka, and a variety of services tail that commit log in order to update caches or respond to changes.  The main services are as follows:
1. api - Exposes a gRPC interface for clients to interact with the data.  Stateless.
2. operations - Defines model objects and transformations on those objects.  Monolithic for now, could be broken out later.  Stateless.
3. objects - Stores a cache of all objects in Redis, indexed by UUID.  Only updates via tailing the commit log in Kafka.  
4. dependencies - Stores a dependency graph between objects in Redis.  Only updates via tailing the commit log in Kafka.
5. submit - Submits changes to the commit log in Kafka.  Updates all dependent objects by calling out to dependencies, objects, and operations.  Stateless.
6. undo - Correlates changes in the commit log into user-defined undo events stored in Redis.  
7. representations - Tails the commit log in Kafka and recalculates client-side representations of changed objects, pushing them to another Kafka topic.  Stateless.
8. geometry-kernel - Hosts an instance of OpenCascade for use in operations and representations.  Stateless.
9. updates - Tails the representations topic in Kafka and pushes them to connected clients via Websocket.  Stateless.

The general control flow goes like this:
1. The user submits a request to change things using api.
2. api collects any necessary information from objects and/or dependencies
3. api calls operations to make the actual change
4. api sends the changes to submit, which pushes them to ObjectState.  It then returns out to the user.
5. objects, dependencies, and undo update caches.
6. representations recalculates client representations and pushes them to ClientRepresentations
6. updates gets the representations from ClientRepresentations and pushes them to clients via Websocket

# Running the application
1. Go to ./ui and run `npm run build`.  
2. Go to this directory and run `docker-compose up -d --build`.  It'll take a while the first time, especially for the geometry kernel.
3. In a browser, go to localhost:8080.
4. Run python scripts in the tests directory.




