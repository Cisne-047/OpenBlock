# OpenBlock — paridad (paper 1.20.4)
# build:  docker build -t paridad .
# run:    docker run -d --name paridad -p 25565:25565 -p 25575:25575 -v paridad-data:/server/world --restart unless-stopped paridad
# (el -v conserva el mundo entre recreaciones del contenedor)
FROM eclipse-temurin:17-jre

WORKDIR /server
COPY . .
RUN chmod +x start.sh install.sh

EXPOSE 25565/tcp
EXPOSE 25575/tcp

# start.sh ya lleva el bucle anti-crash; --restart unless-stopped cubre el resto
ENTRYPOINT ["./start.sh"]
