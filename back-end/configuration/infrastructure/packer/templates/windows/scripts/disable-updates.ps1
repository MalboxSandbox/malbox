# Disable Windows Update for stable analysis environment
Set-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update" -Name "AUOptions" -Value 1
Stop-Service -Name "wuauserv" -Force
Set-Service -Name "wuauserv" -StartupType Disabled

# Disable update orchestrator
Stop-Service -Name "UsoSvc" -Force -ErrorAction SilentlyContinue
Set-Service -Name "UsoSvc" -StartupType Disabled -ErrorAction SilentlyContinue

# Disable Windows Update Medic Service
Stop-Service -Name "WaaSMedicSvc" -Force -ErrorAction SilentlyContinue
Set-Service -Name "WaaSMedicSvc" -StartupType Disabled -ErrorAction SilentlyContinue
