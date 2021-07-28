package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type ConfigTestSuite struct {
	suite.Suite
	backend Backend
	vars    map[string]string
	client  http.Client
}

func (suite *ConfigTestSuite) SetupSuite() {
	suite.vars = make(map[string]string)
}

func (suite *ConfigTestSuite) TestServiceNotFound() {
	upErr := StartProxy("./", "./envoy.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)

	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"does-not-matter"},
	}

	res, resErr := suite.client.Do(req)
	require.Nilf(suite.T(), resErr, "Error creating the HTTP request: %v", resErr)

	var logs []string
	unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
	require.Nilf(suite.T(), resErr, "Error while unmarshaling: %v", unmarshalErr)

	patterns := []string{".*cache miss.*", ".*dispatch successful.*", ".*received response.*", ".*service_token_invalid"}
	patternsMatched := SerialSearch(logs, patterns)

	assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched! Logs: %v", logs)
	StopProxy()
}

func (suite *ConfigTestSuite) TestWrongUpstreamURL() {
	configVars := []byte(`{ 
		"UpstreamURL": "\"http://dogecoin.net:3000\"",
		"ClusterSocketAddress": "dogecoin.net"
	}`)
	configErr := GenerateConfig("temp.yaml", configVars)
	require.Nilf(suite.T(), configErr, "Error generating config file: %v", configErr)

	upErr := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)

	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"does-not-matter"},
	}

	res, resErr := suite.client.Do(req)
	require.Nilf(suite.T(), resErr, "Error creating the HTTP request: %v", resErr)

	var logs []string
	unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
	require.Nilf(suite.T(), resErr, "Error while unmarshaling: %v", unmarshalErr)

	patterns := []string{".*cache miss.*", ".*dispatch successful.*", ".*received response.*", ".*Unexpected characters.*"}
	patternsMatched := SerialSearch(logs, patterns)

	assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched! Logs: %v", logs)
	StopProxy()
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary config file")
}

func (suite *ConfigTestSuite) TestWrongClusterName() {
	configVars := []byte(`{ 
		"ClusterName": "cluster-that-is-not-present"
	}`)
	configErr := GenerateConfig("temp.yaml", configVars)
	require.Nilf(suite.T(), configErr, "Error generating config file: %v", configErr)

	upErr := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)

	require.Eventually(suite.T(), func() bool {
		res, err := http.Get("http://localhost:9095/")
		if err != nil {
			return false
		}
		defer res.Body.Close()
		return true
	}, 15*time.Second, 1*time.Second, "Envoy has not started")
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"does-not-matter"},
	}

	res, resErr := suite.client.Do(req)
	require.Nilf(suite.T(), resErr, "Error creating the HTTP request: %v", resErr)

	var logs []string
	unmarshalErr := json.Unmarshal([]byte(res.Header["Filter-Logs"][0]), &logs)
	require.Nilf(suite.T(), resErr, "Error while unmarshaling: %v", unmarshalErr)

	patterns := []string{".*cache miss.*", ".*BadArgument"}
	patternsMatched := SerialSearch(logs, patterns)

	assert.Equal(suite.T(), true, patternsMatched, "All patterns are not matched! Logs: %v", logs)
	StopProxy()
	deleteErr := os.Remove("./temp.yaml")
	require.Nilf(suite.T(), deleteErr, "Error deleting temporary config file")
}

func TestConfigSuite(t *testing.T) {
	fmt.Println("Running TestConfigSuite")
	suite.Run(t, new(ConfigTestSuite))
}
