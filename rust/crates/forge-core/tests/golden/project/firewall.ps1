# Abre el puerto 25565/tcp en el Firewall de Windows (consola de Administrador)
New-NetFirewallRule -DisplayName "OpenBlock paridad" -Direction Inbound -Protocol TCP -LocalPort 25565 -Action Allow
New-NetFirewallRule -DisplayName "OpenBlock paridad UDP" -Direction Inbound -Protocol UDP -LocalPort 25565 -Action Allow
