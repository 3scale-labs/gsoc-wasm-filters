package main

import (
	"fmt"
	"os"
	"os/exec"
)

func main() {
	fmt.Printf("Test package")
}

// StartContainers runs related docker-compose for the integration tests. This is a helper function for docker container initialization.
func StartContainers(composePath string) error {
	cmd := exec.Command("docker-compose", "-f", composePath, "up", "--build", "--abort-on-container-exit")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	if err != nil {
		return err
	}
	return nil
}

// StopContainers deletes the docker containers created.
func StopContainers(composePath string) error {
	cmd := exec.Command("docker-compose", "-f", composePath, "down")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	if err != nil {
		return err
	}
	return nil
}
