FROM node:latest

COPY ./index.js ./index.js
COPY ./package.json ./package.json 

RUN npm install

EXPOSE 3001
CMD [ "node", "index.js" ]
