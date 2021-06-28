package main

import (
	"fmt"
	"os"
	"os/exec"
)

func main() {
	fmt.Printf("Test package")
}

// StartContainers runs related docker-compose for the integration tests. This is a helper function for docker container initialization. In the case
// of requiring a different docker-compose configuration, create a directory in the configs folder, add related Dockerfile, docker-compose and envoy.yaml.
// For tests that doesn't require a special docker-compose deployment, use BuildnStartContainers() helper to start docker containers by providing
// only a envoy.yaml path.
func StartContainers(composePath string) error {
	cmd := exec.Command("docker-compose", "-f", composePath, "up", "--build", "-d")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	if err != nil {
		fmt.Printf("Error: %v", err)
		return err
	}
	return nil
}

// StopContainers deletes the docker containers created with StartContainers().
func StopContainers(composePath string) error {
	cmd := exec.Command("docker-compose", "-f", composePath, "down")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	if err != nil {
		fmt.Printf("Error: %v", err)
		return err
	}
	return nil
}

// BuildnStartContainers build & start docker containers needed for integration tests. Can be used
// when we want to reuse the same docker-compose.yaml and Dockerfile for tests by providing a dynamic
// build arg to choose envoy.yaml.
func BuildnStartContainers(envoy string) error {
	buildCmd := exec.Command("docker-compose", "build", "--build-arg", fmt.Sprintf("envoy=%s", envoy))
	buildCmd.Stdout = os.Stdout
	buildCmd.Stderr = os.Stderr
	buildErr := buildCmd.Run()
	if buildErr != nil {
		fmt.Printf("Error: %v", buildErr)
		return buildErr
	}
	upCmd := exec.Command("docker-compose", "up", "-d")
	upCmd.Stdout = os.Stdout
	upCmd.Stderr = os.Stderr
	upErr := upCmd.Run()
	if upErr != nil {
		fmt.Printf("Error: %v", upErr)
		return upErr
	}
	return nil
}

// BuildStopContainers stops the containers which are created by BuildnStartContainers.
func BuildStopContainers() error {
	stopCmd := exec.Command("docker-compose", "down")
	stopCmd.Stdout = os.Stdout
	stopCmd.Stderr = os.Stderr
	stopErr := stopCmd.Run()
	if stopErr != nil {
		fmt.Printf("Error: %v", stopErr)
		return stopErr
	}
	return nil
}
