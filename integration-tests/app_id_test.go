package main

import (
	"fmt"
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestAppIdSuccess(t *testing.T) {
	err := StartContainers("docker-compose.yaml")
	if err != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error starting docker-compose: %v", err)

	}

	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	if errReq != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error creating http request: %v", errReq)
	}
	req.Header = http.Header{
		"Host":      []string{"localhost"},
		"x-app-id":  []string{"fcf4db29"},
		"x-app-key": []string{"9a0435ee68f5d647f03a80480a97a326"},
	}
	res, errHTTP := client.Do(req)
	if err != nil {
		fmt.Printf("Error: %v", err)
		t.Fatalf("Error sending http request: %v", errHTTP)
	}
	fmt.Printf("Response: %v", res)
	// assert equality
	assert.Equal(t, 123, 123, "they should be equal")

}
