# Object Definitions

This is where the actual objects that are stored in Model State are defined.  Each object implements what's defined in Object Traits.  Some of these traits are required for all objects passed to Model State, such as traits for identification and dependency updates.  Other traits are not required.  This library is stateless.  It only contains object definitions, so what data an object holds, how it updates itself, how it references other objects, etc.  It depends on Object Traits and Geometry Kernel, but not Operations or Model State.
