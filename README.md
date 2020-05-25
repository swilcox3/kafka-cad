# kafka-cad
![Build](https://github.com/swilcox3/kafka-cad/workflows/Build/badge.svg) ![Unit Tests](https://github.com/swilcox3/kafka-cad/workflows/Unit%20Tests/badge.svg)
This is a cloud-native architectural CAD prototype.  A browser-based UI 


The general idea is that all changes to the model state are stored in Kafka.  Services tail the changes in Kafka and compute results accordingly.  
The general control flow goes like this:
1. The user submits a request to change things using the API.
2. The API server collects any necessary information from caching services
3. The API server calls the service that knows how to make the change
4. The API server sends the changes to the Submit service, which publishes them to the Kafka topic.  It then returns out to the user.
5. Tailing services update any caches and react according to changes.

# Kafka Topics
1. ObjectState - This is the full history of all changes to all objects in the model.
2. ClientRepresentations - Parallels ObjectState with the client representations of all objects, which includes tessellations, drawing views, etc.

# Foundational Services
1. Objects - Holds the changes to all objects in Redis for fast access.  The API is read only, and the cache is only updated through the ObjectState topic.
2. Dependencies - Holds the dependency graphs for all objects in Redis for fast access.  The API is read only, and the cache is only updated through the ObjectState topic.
3. Undo - Holds user undo/redo stacks, mapping change IDs to users.
4. Representations - Calculates the client representations.  Has no external API, only responds to changes in the ObjectState topic and pushes to ClientRepresentations.
5. Updates - Listens to the ClientRepresentations topic and pushes to all connected clients over Websocket.  Externally available.
6. Geometry Kernel - Hosts an instance of OpenCASCADE, for use by Representations and Operational Services
7. Operations - Most actual business logic (Object definitions, update logic, etc.) occurs here.  It has no caches and no state, it is purely functional.  It calls out to the Geometry Kernel, but no other services.
8. Submit - Takes a set of changes as inputs, then gets the list of downstream objects from Dependencies.  It calls out to Operations to recalculate the updated state of all downstream objects, then submits all changes to the ObjectState topic.  
9. Api - Depends on all other services.  This is the high level orchestration, and is externally available.

# Running the application
1. Go to ./ui and run `npm run build`.  
2. Go to this directory and run `docker-compose up -d --build`.  It'll take a while the first time, especially for the geometry kernel.
3. In a browser, go to localhost:8080.




