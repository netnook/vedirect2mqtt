# vedirect2mqtt

This project provides a tool to reading data from [Victron Energy](https://www.victronenergy.com/) devices which provide a [VE.Direct](https://www.victronenergy.com/live/vedirect_protocol:faq) interface (e.g. Victron's MPPT and BMV devices).

## Features

* Reads text encoded data provided by VE.Direct interface (typically sent by devices once per second).
* Sends data an MQTT topic at defined intervals (default every 60 seconds).
* Auto-reconnect to MQTT server.
* Averaging of values where it makes sense (current, voltage).
* Best effort integration of battery current over time - allows approximation of state-of-charge.

## Notably missing

* MQTT TLS support.
* MQTT configuration such as QoS, Will.

## Usage

`vedirect2mqtt --config CONFIG_FILE`

### Config file

Configuration is in TOML.  Fields are required unless stated otherwise. Example:

  [mqtt]
  host = "mqtt.at.home"
  port = 1883 # (optional, default=1883)
  username = "mememe"
  password = "best-kept-secret"
  topic = "my/solar/topic" # (optional, default=vedirect2mqtt)
  
  [device]
  path = "/dev/ttyUSB0"
  
  [publish]
  interval = 300 # (optional, default=60) number of seconds interval between publishing data to mqtt
  

## Status

* This tool is currently in use by me on a Raspberry PI 4 using a Victron USB-to-VE.Direct connector to get data from a MPPT 75/15 solar charge controller and publish to a Mosquitto MQTT server.


