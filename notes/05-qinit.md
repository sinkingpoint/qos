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

## Commands

Qinit will also have some commands. The first we'll implement will be `qinit switchroot [filesystem]` which will:

 - Mount the new file system
 - Create new /dev/ and /proc
 - Change the root to the new filesystem
 - Unmount the existing filesystem
 - Exec qinit in the new filesystem

