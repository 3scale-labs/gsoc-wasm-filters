const express = require("express")
const app = express()
const PORT = 3001

const proxy = require('http-proxy').createProxyServer({
    preserveHeaderKeyCase: true,
    target: "http://listener:3000/"
});

function sleep(ms) {
    return new Promise((resolve) => {
      setTimeout(resolve, ms);
    });
} 

app.all("*", async (req, res, next) => {
    await sleep(6000)
    console.log(`Received request: ${req}`)
    proxy.web(req, res, next);
})

app.listen(PORT,()=>{console.log(`Lstening on port ${PORT}`)})
