package main

import (
	"fmt"
	"net/http"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestAppIdSuccess(t *testing.T) {
	err := StartContainers("./configs/app-id/docker-compose.yaml")
	// Sleep required in github actions to prevent failing of tests in case of services getting late to start.
	time.Sleep(10 * time.Second)
	if err != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error starting docker-compose: %v", err)
	}

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	if errReq != nil {
		fmt.Printf("Error: %v", errReq)
		StopContainers("./configs/app-id/docker-compose.yaml")
		t.Fatalf("Error creating http request: %v", errReq)
	}
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{"fcf4db29"},
		"x-app-key": []string{"9a0435ee68f5d647f03a80480a97a326"},
	}
	res, errHTTP := client.Do(req)
	if errHTTP != nil {
		fmt.Printf("Error: %v", errHTTP)
		StopContainers("./configs/app-id/docker-compose.yaml")
		t.Fatalf("Error sending http request: %v", errHTTP)
	}
	fmt.Printf("Response: %v", res)
	assert.Equal(t, 200, res.StatusCode, "Invalid http response code for appId success test: %v", res.StatusCode)
	assert.Equal(t, "3scale", res.Header.Get("Powered-By"), "Inavlid http header for powered-by: %v", res.Header.Get("Powered-By"))
	errStop := StopContainers("./configs/app-id/docker-compose.yaml")
	if errStop != nil {
		fmt.Printf("Error stopping : %v", errStop)
	}

}

func TestAppIdForbidden(t *testing.T) {
	err := BuildnStartContainers("./configs/app-id/envoy.yaml")
	// Sleep required in github actions to prevent failing of tests in case of services getting late to start.
	time.Sleep(10 * time.Second)
	if err != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error starting docker-compose: %v", err)
	}

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	if errReq != nil {
		fmt.Printf("Error: %v", errReq)
		BuildStopContainers()
		t.Fatalf("Error creating http request: %v", errReq)
	}
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{"fcf4db28"},
		"x-app-key": []string{"9a0435ee68f5d647f03a80480a97a326"},
	}
	res, errHTTP := client.Do(req)
	if errHTTP != nil {
		fmt.Printf("Error: %v", errHTTP)
		BuildStopContainers()
		t.Fatalf("Error sending http request: %v", errHTTP)
	}
	fmt.Printf("Response: %v", res)
	assert.Equal(t, 403, res.StatusCode, "Invalid http response code for appId forbidden test: %d", res.StatusCode)
	assert.Equal(t, "3scale", res.Header.Get("Powered-By"), "Inavlid http header for powered-by: %v", res.Header.Get("Powered-By"))
	errStop := BuildStopContainers()
	if errStop != nil {
		fmt.Printf("Error stopping : %v", errStop)
	}
}

func TestUserKeySuccess(t *testing.T) {
	err := StartContainers("./configs/app-id/docker-compose.yaml")
	// Sleep required in github actions to prevent failing of tests in case of services getting late to start.
	time.Sleep(10 * time.Second)
	if err != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error starting docker-compose: %v", err)
	}
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	if errReq != nil {
		fmt.Printf("Error: %v", errReq)
		StopContainers("./configs/app-id/docker-compose.yaml")
		t.Fatalf("Error creating http request: %v", errReq)
	}
	q := req.URL.Query()
	q.Add("api_key", "46de54605a1321aa3838480c5fa91bcc")
	req.URL.RawQuery = q.Encode()
	res, errHTTP := client.Do(req)
	if errHTTP != nil {
		fmt.Printf("Error: %v", errHTTP)
		StopContainers("./configs/app-id/docker-compose.yaml")
		t.Fatalf("Error sending http request: %v", errHTTP)
	}
	fmt.Printf("Response: %v", res)
	assert.Equal(t, 200, res.StatusCode, "Invalid http response code for user_key success test: %v", res.StatusCode)
	assert.Equal(t, "3scale", res.Header.Get("Powered-By"), "Inavlid http header for powered-by: %v", res.Header.Get("Powered-By"))
	errStop := StopContainers("./configs/app-id/docker-compose.yaml")
	if errStop != nil {
		fmt.Printf("Error stopping : %v", errStop)
	}
}
