# Getty

With an init system, we need something to actually run. `getty` seems like a good choice.

Off the top of my head, getty sets up the terminal, and then runs `login`

https://en.wikipedia.org/wiki/Getty_(Unix)

By "sets up the terminal":

 - Ignore SIGHUP, SIGINT, SIGQUIT (https://github.com/troglobit/getty/blob/master/getty.c#L322-L324)
 - Set Baud rate: https://github.com/troglobit/getty/blob/master/getty.c#L82-L94

What is /etc/issue?

Gets displayed before the login prompt. Basic templating support:

Mine is:

```
04/03/2024 14:49:09 AEDT❯ cat /etc/issue
\S
Kernel \r on an \m (\l)
```

\S >        S or S{VARIABLE}
           Insert the VARIABLE data from /etc/os-release. If this file does not exist then fall back to /usr/lib/os-release. If the VARIABLE argument is not specified, then use PRETTY_NAME from the file or the system name (see \s).
           This escape code can be used to keep /etc/issue distribution and release independent. Note that \S{ANSI_COLOR} is converted to the real terminal escape sequence.

\r is the kernel release
\m is "machine name" (hostname?) - no, its the CPU architecture? Mine is x86_64
\l is the tty name

https://docs.rs/nix/latest/nix/sys/utsname/struct.UtsName.html