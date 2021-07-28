package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"os/exec"
	"text/template"
)

func main() {
	fmt.Printf("Test package")
}

// GenerateConfig uses a config template to generate a config file for the proxy.
func GenerateConfig(name string, jsonData []byte) error {
	tmpl, err := template.ParseFiles("./config_template.yaml")
	if err != nil {
		fmt.Printf("Failed to parse config template: %v", err)
	}

	out := new(bytes.Buffer)
	dataHolder := map[string]interface{}{}
	if err := json.Unmarshal([]byte(jsonData), &dataHolder); err != nil {
		fmt.Printf("Error parsing the json data provided: %v", err)
	}
	if err := tmpl.Execute(out, dataHolder); err != nil {
		fmt.Printf("Failed to fill-in data in the template: %v", err)
	}

	if writeErr := ioutil.WriteFile(name, out.Bytes(), 0777); writeErr != nil {
		fmt.Printf("Error writing temp config file: %v", writeErr)
	}
	return nil
}

// StartProxy build and starts the proxy in a docker container.
func StartProxy(dockerfile string, envoy string) error {
	cmd := exec.Command("sh", "-c", fmt.Sprintf("docker build -t proxy -f %s/Dockerfile --build-arg ENVOY_YAML=%s --no-cache . && docker run -d -p 9095:9095 --network envoymesh --name envoy-proxy-test proxy", dockerfile, envoy))
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Printf("Error starting proxy container: %v", err)
		return err
	}
	return nil
}

// StopProxy stops the container running envoy. Used at the end of each test suite to remove the proxy instance.
func StopProxy() error {
	cmd := exec.Command("sh", "-c", "docker rm -f envoy-proxy-test")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		fmt.Printf("Error removing proxy container: %v", err)
		return err
	}
	return nil
}
