Build an mobile expense tracker that use sms message tracks every transaction made by the user. 


Built an android app to apk converter.
Github Actions CI is the only compile and Build authority, there is no local runtime. By default use Blueprints skill for  managing context. first come up with a plan and write Agent file for cursor. Build our application with aesthetics and intuitive UI. 



considering the following, create agent file, 

- Don't mock up UI / data, build production grade application


## Do not use the word 'NIKE' anywhere in the codebase or during file creation
## If you can't able to access the Github CLI, end the session and I will give you the logs of the CI (Actions)
## Never read `prompts.md`


assemblyDebug

Debug Apk vs 


## How it polls GH CI
```
$id=35880865807; do { Start-Sleep -Seconds 20; $r = Invoke-RestMethod "https://api.github.com/repos/tharunkumar1177/nike-tracker/actions/runs/$id" -Headers @{ 'User-Agent'='cursor' }; Write-Output "$($r.status) $($r.conclusion)" } while ($r.status -ne 'completed'); Invoke-RestMethod "https://api.github.com/repos/tharunkumar1177/nike-tracker/actions/runs/$id/jobs" -Headers @{ 'User-Agent'='cursor' } | ForEach-Object { $_.jobs } | Select-Object name,conclusion | Format-List




$h=@{ 'User-Agent'='cursor' }; Start-Sleep -Seconds 20; $run = (Invoke-RestMethod "https://api.github.com/repos/tharunkumar1177/nike-tracker/actions/runs?per_page=5" -Headers $h).workflow_runs | Where-Object { $_.name -eq 'Release' -and $_.head_branch -eq 'v0.1.1' } | Select-Object -First 1; Write-Output "run: $($run.id) $($run.html_url)"; do { Start-Sleep -Seconds 20; $r = Invoke-RestMethod "https://api.github.com/repos/tharunkumar1177/nike-tracker/actions/runs/$($run.id)" -Headers $h; Write-Output "$($r.status) $($r.conclusion)" } while ($r.status -ne 'completed')

```


To get an installable APK I need to push tag v0.1.1, which runs release.yml against the signing fix and publishes a debug-signed APK.

```
cd C:/nike-tracker; git tag -a v0.1.1 -m "v0.1.1 - debug-signed, sideloadable release APK"; git push origin v0.1.1
```



AI feature - chatbot X


Location based transaction tagging


Do not use the word 'NIKE' anywhere in the codebase


.cursor/terminal/ ...45kb file - why rate limit hit?







"Not a real expense? Ignore it" - even for the "income", its showing the "expense" in the review section.

---

addressing this problem, add a card right after the 'transaction details' with toggle button for enabling/counting as income/expense.

```
________________________________________
|time, direction, Account               |
|                                       |
----------------------------------------
_________________________________________________________
|(card/mode of transaction)  Income/ Expense <toggle>   |
---------------------------------------------------------


```
