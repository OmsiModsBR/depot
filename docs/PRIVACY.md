# Privacy

Project Depot collects no data about you. It has no telemetry, no analytics, no account,
no identifier of your installation or session, and nothing that reports that you are playing.
Settings, logs, screenshots and everything else the program writes stay on your computer, under
`~/.depot` (on Windows `C:\Users\<you>\.depot`) and the game folders you choose.

## When the program does use the network

Only for something you asked for, and only to the host named here. Nothing is sent that
identifies you beyond what any web request carries (your IP address and the program's name and version).

| What | When | Host |
| --- | --- | --- |
| Internet radio of a bus | A bus radio is switched on; the stations are in `~/.depot/radio.cfg` (a map or radio plugin may add its own). Only `https://` addresses of public hosts. | the station's own server |
| Real weather (METAR) | You pick it in the weather menu; the station code is sent, nothing else. | `aviationweather.gov` |
| Downloads of the hidden original OMSI 2 mode | You confirm an install or update of OmsiLaunch or a module. | `github.com` (the project's own releases) |
| Links | You click one; it opens in your browser. | the project's GitHub pages, or Microsoft's .NET download page |

## Steam

On builds that include the Steam library (Windows x64, Linux x64, macOS), the program connects to
the Steam client **if it is running**, as the game with Steam app id 252530. Steam itself then shows
that you are playing, to the people Steam lets see it. Depot sends no data of its own through it
and does nothing when Steam is not running.

The program does not check for updates, does not install anything by itself and has no
multiplayer or "playing now" feature: those were removed (see [CHANGELOG](../CHANGELOG.md)).

## Logs and problem reports

`game.log` and `launcher.log` are written locally and are replaced on each run. They can contain
paths on your computer (including your Windows user name), the graphics card and the maps and
vehicles you loaded. **Nothing sends them.** "Copy report" puts the end of the log on your
clipboard, and "Report a problem" opens a new GitHub issue in your browser with a text you can read
and edit; nothing is posted until you submit it yourself. Check the text for personal paths first.

## The website

The project's website is static, with no cookies or analytics. Opening it makes your browser
contact two third parties, which see your IP address:

* **GitHub** (`api.github.com`), to read the latest release and the issue list.
* **Google Fonts** (`fonts.googleapis.com`, `fonts.gstatic.com`), to load the page's fonts. We
  intend to host the fonts ourselves so the page no longer calls Google; this note will change when it does.

## Questions

Open an issue on the project's page, or see [SECURITY.md](../SECURITY.md) for a problem with how
the program handles your data.
