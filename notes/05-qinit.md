# QInit


## Data Design

We're building an init system! Our init system will have one job: work out what services to run, and then run them. Service definitions will look like (stealing heavily from systemd):

```
[Service]
Command = /sbin/login ${TTY} 
[[Arguments]]
Name = "TTY"
Required = true

Depends = [ "Other Services" ]
```

Where `Command` is the command to run, which can be templated with arguments in `Arguments`. Systemd supports this somewhat with @ notation, like `login@tty1.service`, but I'd like to be a bit more explicit.

Services will be able to be grouped together into "Spheres". Spheres will look like this:

```
[Sphere]
[[Services]]
Name = "login"
[[Services.Variables]]
TTY = "/dev/ttys0"
```

When a sphere is started, it will start all the services defined in that sphere.

## Dependency resolving

Services are (needs, service) tuples. To start service a:

 - Construct a directed graph of all the needs
 - Walk that graph and flatten it into an array, ordered by the depth in the graph

e.g.

service (needs)

a (b)
b (c)
c ()

Graph: a -> b -> c , flattened: [c, b, a] (start c, then b, then a)

a (b, c)
b (d)
c (d)
d ()

graph:
  b
 / \
a   d
 \ /
  c

flattened: [d, c, b, a], where c, and b are interchangable

  b
 /
a 
 \
  c - d - e

(b depends on d)

flattened: [a, b, c, d]

https://en.wikipedia.org/wiki/Topological_sorting