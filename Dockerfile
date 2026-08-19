FROM debian:11-slim

RUN apt update
RUN apt install -y curl 

USER root

RUN apt update
RUN apt install -y guacd curlftpfs libc6 ca-certificates curl gnupg icoutils
RUN apt purge -y nodejs-legacy
RUN mkdir -p /etc/apt/keyrings
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg
RUN echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list
RUN apt update
RUN apt install nodejs -y

#RUN npm install -g --force npm@latest
RUN npm install -g nodemon


WORKDIR /opt/server

CMD nodemon --ignore dist ./index.js
