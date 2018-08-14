# WIP

## A crate to interact with the chrome developer tools

build.rs basically transforms the developer tools protocol spec (src/chrome_protocol.json) into rust code.

Right now it generates an enum and a synchronous api, there is no reason why it can't generate something else.

## Usage

Start chromium

```
$ chromium --remote-debugging-port=9222
```

