# kafka-cad
I've gotten far enough on my monolithic application I think I can break it out into microservices intelligently now.  The general idea is that all changes to the model state are stored in Kafka.  Services tail the changes in Kafka and compute results accordingly.  
The general control flow goes like this:
1. The user submits a request to change things in the API.
2. The API server directs it to the appropriate service that knows how to apply the change.  
3. That service collects any necessary information, transforms the data, and writes the change to the appropriate Kafka topic, and returns any values to the API server.
4. The API server returns to the client.
5. Tailing services update any caches and react according to changes.

For example, let's follow a move objects operation:
1. The user makes a MoveObjects gRPC call.  It contains a list of object IDs and the delta vector, as well as the change ID.
2. The API server directs the message to the Position service.  
3. The Position service calls the GetObjects function on the Objects service with the list of IDs and gets the state of the objects at that change ID.  
4. The Position service updates the state of each object to be moved by the delta vector.
5. The Position service pushes the changed object states to the ObjectState topic in Kafka.
6. The Position service returns success and the new change ID, which gets passed back to the client.
7. The Objects service sees the new change in the ObjectState topic and updates its cache.
8. The Representation service sees the new change in the ObjectState topic, calculates new client representations of all included objects, and pushes them to the ClientRepresentations topic.
9. The Updates service sees the new representations in the ClientRepresentations topic, and pushes them via Websocket to all connected clients.

# Kafka Topics
1. ObjectState - This is the full history of all changes to all objects in the model.
2. ClientRepresentations - Parallels ObjectState with the client representations of all objects, which includes tessellations, drawing views, etc.

# Foundational Services
1. Objects - Holds the last few changes to all objects in Redis for fast access.  The API is read only, and the cache is only updated through the ObjectState topic.
2. Dependencies - Holds the dependency graphs for the last few changes in Redis for fast access.  The API is read only, and the cache is only updated through the ObjectState topic.
3. Undo - Holds user undo/redo stacks, mapping change IDs to users.
4. Representations - Calculates the client representations.  Has no external API, only responds to changes in the ObjectState topic and pushes to ClientRepresentations.
5. Updates - Listens to the ClientRepresentations topic and pushes to all connected clients over Websocket.
6. Geometry Kernel - Hosts an instance of OpenCASCADE, for use by Representations and Operational Services




