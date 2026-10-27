@echo off
rem Build DyeTrace (x86 GUI).
setlocal
set HERE=%~dp0

if defined VCINSTALLDIR goto have_vc
set "VCVARS=C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars32.bat"
if exist "%VCVARS%" goto do_vcvars
set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars32.bat"
if exist "%VCVARS%" goto do_vcvars
echo Could not find vcvars32.bat - run from an "x86 Native Tools" prompt.
exit /b 1

:do_vcvars
call "%VCVARS%" >nul

:have_vc
cl /nologo /O2 /W3 /D_CRT_SECURE_NO_WARNINGS "%HERE%dyetrace.c" ^
   /Fe"%HERE%dyetrace.exe" /link /SUBSYSTEM:WINDOWS user32.lib gdi32.lib
if errorlevel 1 exit /b 1

echo.
echo Built: %HERE%dyetrace.exe
echo Self test: dyetrace.exe --selftest
endlocal
