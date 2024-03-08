# Crond

This is my take on a cron alternative
It executes powershell scripts (requires pwsh)

## Usage

put a toml file like this in ~/.crond.toml or ~/.config/crond.toml

```toml
[[entries]]
command = """
cd '/repos/sockudo'
git fetch
cargo build
cargo build -r
"""
duration = "00:02:00"

[[entries]]
command = """
cd "/repos"
$repoDir = "foo"  

function Run-Maintenance {
    param([string]$repoPath)

    cd $repoPath  
    git maintenance run 
    cd ..  
}
Get-ChildItem -Path $repoDir -Directory -Filter ".git" -Recurse | 
    ForEach-Object { Run-Maintenance $_.FullName } 
"""
duration = "01:00:00"
```
