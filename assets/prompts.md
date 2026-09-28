Build an mobile expense tracker that use sms message tracks every transaction made by the user. 


Built an android app to apk converter.
Github Actions CI is the only compile and Build authority, there is no local runtime. By default use Blueprints skill for  managing context. first come up with a plan and write Agent file for cursor. Build our application with aesthetics and intuitive UI. 



considering the following, create agent file, 


# Guardrails

- Don't mock up UI / data, build production grade application
- Do not use the word 'NIKE' anywhere in the codebase or during file creation
- If you can't able to access the Github CLI, end the session and I will give you the logs of the CI (Actions)
- Never read `prompts.md`


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
















```
Remove-Item -Recurse -Force .git

rmdir /s /q .git

git init
git log --oneline

```


echo "# Nike-op" >> README.md
git init
git add README.md
git commit -m "first commit"
git branch -M main
git remote add origin https://github.com/tharunkumar1177/Nike-op.git
git push -u origin main






I want to publish this optimizer in MS store. Is there any optimal way of installing this application that put all the core components into one installed exe.

If there is any architectural flaws/quirks - come up with a plan. Only proceeed further if it is approved. 
you can always propose optimal implementation than the blueprint , only proceed further if user say approved it

This project right now has many flaws, a lot features / functionalities isn't working as expected. instead of directly editing it, ask questions of "how it could have been implemented better?" or "Stick with the existing system."


Stick with the existing Privileged broker and cleanup [as described in blueprints].
For [clean up for browser, system, temp files and so on] clearing system-level files - on a scheduled date and time. I think there is no UI for user to set these things up. so Create one




and installs a network-routing driver (sometimes needing a restart) plus the app running with admin rights.





