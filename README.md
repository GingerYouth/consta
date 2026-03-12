# ***Con***tribution ***Sta***tistics
CLI tool to get git contribution statistics for multiple repos. The project was born from the desire to track this stats for non-github/gitlab projects.

![output.png](output.png)
![grid.png](grid.png)

## Installation and launching
|                                   | Installation                                                                                                              | Launching                                                                    |
|-----------------------------------|---------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------|
| If you have cargo (_recommended_) | `cargo install consta`                                                                                                    | `cargo ...`                                                                  |
| If you don't have cargo           | Download the latest release for your platform from [releases page](https://github.com/GingerYouth/consta/releases/latest) | Run executable as you would do in your OS, e.g.: `consta.exe ...` on Windows |

## Examples
1) Specified repositories:
```
consta "C:\projects\project-name-1" "C:\projects\project-name-2" --author "andrei" --since "2026-01-01" --breakdown --recursive
```
2) Recursive local repositories search:
```
consta "C:\projects\" --author "andrei" --since "2026-01-01" --breakdown --recursive
```
3) Local and remote repositories with different author patterns:
```
consta "C:\projects\project-name-1" "https://github.com/GingerYouth/consta" --author "andrei" --author "GingerYouth" --token "github_pat_..."
```

## CLI arguments:
* *author* \<string>      : Filter commits by author name or email (multiple allowed to search for different patterns for local and remote repos)
* *since* \<date>         : Show commits after this date (e.g. 2026-01-01)
* *until* \<date>         : Show commits before this date
* *breakdown*             : Show commit breakdown
* *grid* \<year>          : Show activity grid for a specific year
* *recursive* \[\<depth>] : Discover repos in subdirectories (default depth = 3)
* *token* \<string>       : GitHub personal access token