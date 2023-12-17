# Export Signal Desktop To Json
A simple app for exporting Signal desktop messages from their SQLite database.

## How to use
```
# Currently supports either environment variables or command line arguments
./export-signal-desktop-to-json --config-path "C:\Users\ck\AppData\Roaming\Signal\config.json" --database-path "C:\Users\ck\AppData\Roaming\Signal\sql\db.sqlite" --output-directory "D:\code\test"
# Alternately set SIGNAL_CONFIG_PATH SIGNAL_DATABASE_PATH SIGNAL_OUTPUT_DIRECTORY

```
## What's not working yet 
* Attachment support


## Why make this? 
I was recently irked when Signal recently started crash looping on my phone and the only reasonable way to fix it seemed to be a reinstall.
At the time it looked like it was going to wipe all my data, and only some phones support backups of Signal data - I get it, this is meant to be a secure messenger, but only having some partial backup support makes no sense to me.
I took a look at some other repos and they involved a fairly cumbersome docker process and none of them seemed to be working at the moment. 


## Notes on Signal backups

After taking a look at the various implementations to read the backups I found that Signal uses a custom extension of SQLite to store their data securely.
However, the key to storing the data is stored in plain text on disk in the config.json file.

Really, this data is not secured from anything but the most casual review, so I don't see the point of making it so difficult to access.
