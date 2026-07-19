@echo off
rem RutBusiness Server — arranca pharma-api con la DB del worktree.
rem Sin PHARMA__DB__PATH absoluto el server ancla la DB relativa a
rem C:\ProgramData\PharmaServer\data (install dir) y abre otra base.
set "PHARMA__DB__PATH=D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2\data\surreal"
cd /d "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2"
"D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2\target\release\pharma-api.exe"
