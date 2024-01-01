# Creating a shell

A shell sounds like something useful to have. Something that I can type commands into and run binaries. Let's explore what's required here.

Off the bat, I'm gonna need a parser - https://pest.rs/

Maybe stty settings? - that'll need ioctls

https://ftp.gnu.org/old-gnu/Manuals/glibc-2.2.3/html_chapter/libc_17.html
https://ftp.gnu.org/old-gnu/Manuals/glibc-2.2.3/html_chapter/libc_toc.html#TOC578

> When a shell program that normally performs job control is started, it has to be careful in case it has been invoked from another shell that is already doing its own job control. 

We'll need a process group

getpgid/setpgid etc

https://docs.rs/nix/latest/nix/ / https://github.com/nix-rust/nix


https://www.gnu.org/software/libc/manual/html_node/Job-Control-Signals.html - SIGTTIN

---

```
{"msg":"failed to run tcgetpgrp. Err: -ENOTTY: Not a typewriter","level":"ERRO","ts":"2023-12-29T07:22:16.983349661Z"}
```

Hrmm https://pubs.opengroup.org/onlinepubs/009696699/functions/tcgetpgrp.html

> The calling process does not have a controlling terminal, or the file is not the controlling terminal.

What's a controlling terminal?

Seems like `sh` does this as well, so probably innocuous.

```
init: cannot set terminal process group (-1): Inappropriate ioctl for device
init: no job control in this shell
```

https://gist.github.com/notheotherben/819ad3a3ada4a05e6fcbd9fcb27a992f

---

Parsing:

PEG Parser? https://pest.rs

Something like:

```
alpha        =  { 'a'..'z' | 'A'..'Z' }
digit        =  { '0'..'9' }
double_quote =  { "\"" }
escape       =  { "\\" }
COMMENT      = @{ "#" ~ (!"\n" ~ ANY)* }
WHITESPACE   = _{ " " }

expression = { SOI ~ env_variable* ~ string* EOI }

env_variable     =  { env_variable_key ~ "=" ~ string }
env_variable_key = @{ alpha ~ (alpha | digit | "_")* }

string = { unquoted_string | single_quoted_string | double_quoted_string }

unquoted_string = @{ (unquoted_char | escaped_char)+ }
unquoted_char   = @{ !(('\u{00}'..'\u{20}') | "\"" | "'" | "\\") ~ ANY }

single_quoted_string         = @{ "'" ~ (single_quoted_escaped_char | single_quoted_unescaped_char)* ~ "'" }
single_quoted_escaped_char   = @{ escaped_char | (escape ~ "'") }
single_quoted_unescaped_char = @{ !('\u{00}'..'\u{1f}' | "\'" | "\\") ~ ANY }

double_quoted_string         = @{ "\"" ~ (double_quoted_escaped_char | double_quoted_unescaped_char)* ~ "\"" }
double_quoted_escaped_char   = @{ escaped_char | (escape ~ "\"") }
double_quoted_unescaped_char = @{ !('\u{00}'..'\u{1f}' | "\"" | "\\") ~ ANY }

escaped_char = @{ escape ~ ("\\" | "b" | "f" | "n" | "r" | "t" | ("u" ~ ASCII_HEX_DIGIT{2, 4})) }
```

Let's build our own.


Stdin is buffered by default - need to enable "non canonical mode": https://www.gnu.org/software/libc/manual/html_node/Noncanon-Example.html

---

Escape sequences

Right key:

char_buffer: [27] = ESC
char_buffer: [91] = [
char_buffer: [67] = C

https://en.wikipedia.org/wiki/ANSI_escape_code

```
Code 	Abbr 	Name 	Effect
CSI n A 	CUU 	Cursor Up 	Moves the cursor n (default 1) cells in the given direction. If the cursor is already at the edge of the screen, this has no effect.
CSI n B 	CUD 	Cursor Down
CSI n C 	CUF 	Cursor Forward
CSI n D 	CUB 	Cursor Back
CSI n E 	CNL 	Cursor Next Line 	Moves cursor to beginning of the line n (default 1) lines down. (not ANSI.SYS)
CSI n F 	CPL 	Cursor Previous Line 	Moves cursor to beginning of the line n (default 1) lines up. (not ANSI.SYS)
CSI n G 	CHA 	Cursor Horizontal Absolute 	Moves the cursor to column n (default 1). (not ANSI.SYS)
CSI n ; m H 	CUP 	Cursor Position 	Moves the cursor to row n, column m. The values are 1-based, and default to 1 (top left corner) if omitted. A sequence such as CSI ;5H is a synonym for CSI 1;5H as well as CSI 17;H is the same as CSI 17H and CSI 17;1H
CSI n J 	ED 	Erase in Display 	Clears part of the screen. If n is 0 (or missing), clear from cursor to end of screen. If n is 1, clear from cursor to beginning of the screen. If n is 2, clear entire screen (and moves cursor to upper left on DOS ANSI.SYS). If n is 3, clear entire screen and delete all lines saved in the scrollback buffer (this feature was added for xterm and is supported by other terminal applications).
CSI n K 	EL 	Erase in Line 	Erases part of the line. If n is 0 (or missing), clear from cursor to the end of the line. If n is 1, clear from cursor to beginning of the line. If n is 2, clear entire line. Cursor position does not change.
CSI n S 	SU 	Scroll Up 	Scroll whole page up by n (default 1) lines. New lines are added at the bottom. (not ANSI.SYS)
CSI n T 	SD 	Scroll Down 	Scroll whole page down by n (default 1) lines. New lines are added at the top. (not ANSI.SYS)
CSI n ; m f 	HVP 	Horizontal Vertical Position 	Same as CUP, but counts as a format effector function (like CR or LF) rather than an editor function (like CUD or CNL). This can lead to different handling in certain terminal modes.[5]: Annex A 
CSI n m 	SGR 	Select Graphic Rendition 	Sets colors and style of the characters following this code
CSI 5i 		AUX Port On 	Enable aux serial port usually for local serial printer
CSI 4i 		AUX Port Off 	Disable aux serial port usually for local serial printer

CSI 6n 	DSR 	Device Status Report 	Reports the cursor position (CPR) by transmitting ESC[n;mR, where n is the row and m is the column. 
```

---

Buffering?

Output is only happening when I press enter

Doesn't look like line buffering - `flush` doesn't work.

Disabling "Canonical mode" seems to be the fix

---

Rendering in the middle of a line?

buffer = ['f', 'o', 'o']
cursor = 1 (over the first o)

Inserts should happen _before_ the cursor
So pressing 'i' should make ['f', 'i', 'o', 'o'] with cursor = 2

Logic:

 - Insert i (cursor = 2, terminal pos = 2)
 - flush from (cursor-1) -> eol (display is 'fi')
 - rewrite cursor + 1 -> eol (display is 'fioo', cursor=1, terminal pos=4)
 - move cursor back (length - cursor + 1 = 4 - 1 + 2 = 2, terminal_pos = 2)

Backspace?


buffer = ['f', 'o', 'b']
cursor = 2 (over the first o)

