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

So getty reads the username, and passes the username to `login`, and then `login` disables echoing and reads the password.

So `login` is responsible for /etc/shadow parsing. Is it setuid? no, it just runs as root as inheritted by getty

## /etc/shadow format

So login has to validate the password. How does it do that?

```
bin:*:18656:0:99999:7:::
daemon:*:18656:0:99999:7:::
```
https://linux.die.net/man/5/shadow

Colon delimitted file

1. username
2. password - what do ! and * mean? - anything not valid crypt(3) format means you can't log in with a password. Empty means no password required
3. "last changed" - unix timestamp of last changed time
4. minimum number of days between password changes
5. maximum number of days between password changes
6. password warning
7. days after expiry that the password is still accepted
8. account expiration days

what is /etc/gshadow??

Also colon delimitted.

1. group name
2. encrypted password - wat. groups can have passwords??
3. comma seperated list of "administrators"
4. comma seperated list of members

`newgrp` - change your primary group. if you're not a member of the group then you can enter the `group password" to join I guess?

Passwod looks like `$<number>$salt$<hash>`

1. md5
2a. blowfish
5. sha-256
6. sha-512

### md5

https://github.com/ewxrjk/crypt3/blob/master/libcrypt3/crypt-md5.c#L57-L76

MD5 salt is password + "$1$" + salt + password + salt + password

https://akkadia.org/drepper/SHA-crypt.txt


wtf is `yescrypt`?

created a new user and got:

test:$y$j9T$CrvjUO9SCMVGv5kTJe0Id/$dqScmjExOtXsgL1T38bYhWQ8puN18OKD1p4LEalXiy2:19788:0:99999:7:::

with password `test`

shitty stub article: https://en.wikipedia.org/wiki/Yescrypt

> yescrypt is a cryptographic hash function used for password hashing on Fedora,[1] Debian,[2] Ubuntu,[3] and Arch Linux.[4] The function is more resistant to offline password-cracking attacks than SHA-512.[5] 

https://www.baeldung.com/linux/shadow-passwords

> In all cases except DES, the whole format of the field becomes more complex:
> $id$param$salt$encrypted

https://unix.stackexchange.com/questions/690679/what-does-j9t-mean-in-yescrypt-from-etc-shadow

Apparently $7$ _also_ means yescrypt?

  $7$ - classic scrypt hashes, not very compact fixed-length encoding
  $y$ - native yescrypt and classic scrypt hashes, new extremely compact variable-length encoding

Crate is empty and abandoned by the RustCrypto project: https://crates.io/crates/yescrypt

https://github.com/defuse/yescrypt/blob/master/new-spec/yescrypt.txt

kind of crazy this is the default with basically no documentation