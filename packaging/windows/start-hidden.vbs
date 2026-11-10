Option Explicit
Dim shell, files, appDir, executable
Set shell = CreateObject("WScript.Shell")
Set files = CreateObject("Scripting.FileSystemObject")
appDir = files.GetParentFolderName(WScript.ScriptFullName)
executable = files.BuildPath(appDir, "layout-keeper.exe")
shell.CurrentDirectory = appDir
' Window style 0 hides the console; False keeps startup non-blocking.
shell.Run Chr(34) & executable & Chr(34), 0, False
