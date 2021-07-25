package main

import (
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"github.com/stretchr/testify/suite"
)

type SingletonFlushTestSuite struct {
	suite.Suite
	ServiceID    string
	ServiceToken string
	AppIDs       []string
	PlanID       string
	Metrics      []Metric
}

func (suite *SingletonFlushTestSuite) SetupSuite() {
	suite.ServiceID = "test-service-id"
	suite.AppIDs = []string{"test-app-id-1", "test-app-id-2"}
	suite.ServiceToken = "test-service-token"
	suite.PlanID = "test-plan-id"
	suite.Metrics = []Metric{
		{"hits", "1", []UsageLimit{
			{Day, 10000},
		}},
		{"rq", "2", []UsageLimit{
			{Month, 10000},
		}},
	}

}

func (suite *SingletonFlushTestSuite) BeforeTest(suiteName, testName string) {
	if testName == "TestSingletonContainerFlush" {
		configErr := configureSingletonFlush("ContainerLimit")
		require.Nilf(suite.T(), configErr, "Error configuring envoy.yaml for container flush : %v", configErr)

		require.Eventually(suite.T(), func() bool {
			fmt.Printf("Creating service with service_id: %s, service_token:%s", suite.ServiceID, suite.ServiceToken)
			serviceErr := CreateService(suite.ServiceID, suite.ServiceToken)
			if serviceErr != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error creating the service")

		for _, app := range suite.AppIDs {
			require.Eventually(suite.T(), func() bool {
				err := AddApplication(suite.ServiceID, app, suite.PlanID)
				if err != nil {
					return false
				}
				return true
			}, 4*time.Second, 500*time.Millisecond, "Error creating the app")
		}

		require.Eventually(suite.T(), func() bool {
			err := AddMetrics(suite.ServiceID, &suite.Metrics)
			if err != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error adding metrics")

		require.Eventually(suite.T(), func() bool {
			err := UpdateUsageLimits(suite.ServiceID, suite.PlanID, &suite.Metrics)
			if err != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error adding usage limits")
	}

}

func configureSingletonFlush(flushMode string) error {
	configData := []byte(fmt.Sprintf(`{ "SingletonFlushMode": "%s" }`, flushMode))
	return GenerateConfig("temp.yaml", configData)
}

func (suite *SingletonFlushTestSuite) TestSingletonContainerFlush() {
	upErr := StartProxy("./", "./temp.yaml")
	require.Nilf(suite.T(), upErr, "Error starting proxy: %v", upErr)
	client := &http.Client{}
	req, errReq := http.NewRequest("GET", "http://127.0.0.1:9095/", nil)
	require.Nilf(suite.T(), errReq, "Error creating the HTTP request: %v", errReq)
	req.Header = http.Header{
		"Host":     []string{"localhost"},
		"x-app-id": []string{"test-app-id-1"},
	}
	for i := 0; i < 4; i++ {
		res, _ := client.Do(req)
		fmt.Printf("Response: %v\n", res)
	}
	time.Sleep(3 * time.Second)
	usage, usageErr := getApisonatorUsage("test-service-id", "test-service-token", "test-app-id-1")
	require.Nilf(suite.T(), usageErr, "Error fetching usages from apisonator: %v", usageErr)
	require.Equal(suite.T(), usage.Current, int64(4), "Invalid number for usages for the metric hits in apisonator")
	downErr := StopProxy()
	require.Nilf(suite.T(), downErr, "Error stopping proxy: %v", downErr)
}

func TestSingletonFlushSuite(t *testing.T) {
	fmt.Println("Running SingletonFlushSuite")
	suite.Run(t, new(SingletonFlushTestSuite))
}

func getApisonatorUsage(serviceID string, serviceToken string, appID string) (UsageReport, error) {
	client := &http.Client{}
	req, reqErr := http.NewRequest("GET", "http://127.0.0.1:3000/transactions/authorize.xml", nil)
	if reqErr != nil {
		fmt.Printf("reqErr: %v\n", reqErr)
		return UsageReport{}, reqErr
	}
	q := req.URL.Query()
	q.Add("service_token", serviceToken)
	q.Add("service_id", serviceID)
	q.Add("app_id", appID)
	req.URL.RawQuery = q.Encode()
	res, resErr := client.Do(req)
	if resErr != nil {
		fmt.Printf("resErr: %v\n", resErr)
		return UsageReport{}, resErr
	}
	fmt.Printf("Auth response: %v\n", res)
	response := AuthResponse{}
	bodyBytes, _ := io.ReadAll(res.Body)
	xmlErr := xml.Unmarshal(bodyBytes, &response)
	if xmlErr != nil {
		fmt.Printf("XML Error: %v\n", xmlErr)
	}
	fmt.Printf("XML: %v\n", response)
	return response.Usages[0], nil
}

func (suite *SingletonFlushTestSuite) AfterTest(suiteName, testName string) {
	if testName == "TestSingletonContainerFlush" {
		require.Eventually(suite.T(), func() bool {
			fmt.Printf("Deleting service with service_id: %s, service_token:%s\n", suite.ServiceID, suite.ServiceToken)
			serviceErr := DeleteService(suite.ServiceID, suite.ServiceToken)
			if serviceErr != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error deleting the service")

		for _, app := range suite.AppIDs {
			require.Eventually(suite.T(), func() bool {
				err := DeleteApplication(suite.ServiceID, app)
				if err != nil {
					return false
				}
				return true
			}, 4*time.Second, 500*time.Millisecond, "Error deleting the app")
		}

		require.Eventually(suite.T(), func() bool {
			err := DeleteMetrics(suite.ServiceID, &suite.Metrics)
			if err != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error deleting metrics")

		require.Eventually(suite.T(), func() bool {
			err := DeleteUsageLimits(suite.ServiceID, suite.PlanID, &suite.Metrics)
			if err != nil {
				return false
			}
			return true
		}, 4*time.Second, 500*time.Millisecond, "Error deleting usage limits")
		deleteErr := os.Remove("./temp.yaml")
		require.Nilf(suite.T(), deleteErr, "Error deleting temporary envoy.yaml")
	}
}
