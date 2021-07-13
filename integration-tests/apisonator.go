package main

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"net/http"
)

// These credentials should match those mentioned in the ci.yaml.
const (
	InternalUser   = "root"
	InternalPass   = "root"
	Port           = "3000"
	IPAddress      = "0.0.0.0"
	InternalPrefix = "/internal"
	InternalURL    = "http://" + InternalUser + ":" + InternalPass + "@" + IPAddress + ":" + Port + InternalPrefix
)

// Period represents the time period for metrics
type Period string

const (
	// Minute represents a minute in time.
	Minute Period = "minute"
	// Hour represents a hour in time.
	Hour Period = "hour"
	// Day represents a day in time.
	Day Period = "day"
	// Week represents a week in time.
	Week Period = "week"
	// Month represents a month in time.
	Month Period = "month"
	// Year represents a year in time.
	Year Period = "year"
	// Eternity represents forever.
	Eternity Period = "eternity"
)

func (p Period) String() string {
	return string(p)
}

// UsageLimit in threeescale.
type UsageLimit struct {
	period Period
	value  int
}

// Metric in threescale.
type Metric struct {
	name   string
	id     string
	limits []UsageLimit
}

// CreateService helper creates a service in the threescale.
func CreateService(serviceID string, serviceToken string) error {
	client := &http.Client{}
	// creating service with specified service_id
	headerData := []byte(fmt.Sprintf(`
		{ 
			"service": {
				"id": "%s",
				"provider_key":"my_provider_key",
				"state": "active"
			}
		}`, serviceID))
	req, err := http.NewRequest("POST", InternalURL+"/services/", bytes.NewBuffer(headerData))
	if err != nil {
		fmt.Printf("Error while creating HTTP request: %v", err)
		return err
	}
	res, err := client.Do(req)
	if err != nil {
		fmt.Printf("Error sending the HTTP request: %v", err)
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create a new service(id: %s)", serviceID)
	}

	// adding a service_token to previously created service
	headerData = []byte(fmt.Sprintf(`
		{ 
			"service_tokens": {
				"%s": {
					"service_id": "%s"
				}
			}
		}`, serviceToken, serviceID))
	url := InternalURL + "/service_tokens/"
	res, err = executeHTTPRequest(http.MethodPost, url, &headerData)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create a service id (%s) and token pair (%s)", serviceID, serviceToken)
	}
	return nil
}

// DeleteService deletes a service
func DeleteService(serviceID string, serviceToken string) error {
	url := InternalURL + "/services/" + serviceID
	res, err := executeHTTPRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the service(id: %s)", serviceID)
	}
	headerData := []byte(fmt.Sprintf(`
		{ 
			"service_tokens": [{
				"service_token": "%s",
				"service_id": "%s"
			}]
		}`, serviceToken, serviceID))
	url = InternalURL + "/service_tokens/"
	res, err = executeHTTPRequest(http.MethodDelete, url, &headerData)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the service id (%s) and token pair (%s)", serviceID, serviceToken)
	}
	return nil
}

// AddApplication creates a new application associated with 'service_id', 'app_id' and 'plan_id'
func AddApplication(serviceID string, appID string, planID string) error {
	headerData := []byte(fmt.Sprintf(`
		{ 
			"application": {
				"service_id": "%s",
				"id": "%s",
				"plan_id": "%s",
				"plan_name": "Basic",
				"state": "active"
			}
		}`, serviceID, appID, planID))
	url := InternalURL + "/services/" + serviceID + "/applications/" + appID
	res, err := executeHTTPRequest(http.MethodPost, url, &headerData)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to create an application(id: %s)", appID)
	}
	return nil
}

// DeleteApplication deletes an application.
func DeleteApplication(serviceID string, appID string) error {
	url := InternalURL + "/services/" + serviceID + "/applications" + serviceID
	res, err := executeHTTPRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the application(service_id: %s, app_id: %s)", serviceID, appID)
	}
	return nil
}

// AddApplicationKey adds key to the application identified by 'service_id' and 'app_id'
func AddApplicationKey(serviceID string, appID string, key string) error {
	headerData := []byte(fmt.Sprintf(`
		{ 
			"application_key": {
				"value": "%s"
			}
		}`, key))
	url := InternalURL + "/services/" + serviceID + "/applications/" + appID + "/keys/"
	res, err := executeHTTPRequest(http.MethodPost, url, &headerData)
	if err != nil {
		return err
	}
	if res.StatusCode != 201 {
		return fmt.Errorf("Failed to add an application key(app_id: %s; key: %s)", appID, key)
	}
	return nil
}

// DeleteApplicationKey deletes an application key
func DeleteApplicationKey(serviceID string, appID string, key string) error {
	url := InternalURL + "/services/" + serviceID + "/applications" + appID + "/keys/" + key
	res, err := executeHTTPRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete the application key(app_id: %s, key: %s)", appID, key)
	}
	return nil
}

// AddUserKey adds a user key to the specified application.
func AddUserKey(serviceID string, appID string, key string) error {
	url := InternalURL + "/services/" + serviceID + "/applications/" + appID + "/key/" + key
	res, err := executeHTTPRequest(http.MethodPut, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to add a user key(app_id: %s)", appID)
	}
	return nil
}

// DeleteUserKey deletes a user key
func DeleteUserKey(serviceID string, appID string, key string) error {
	url := InternalURL + "/services/" + serviceID + "/applications/key" + key
	res, err := executeHTTPRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to delete a user key for app(id: %s)", appID)
	}
	return nil
}

// AddMetrics adds a metrics to a service
func AddMetrics(serviceID string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		headerData := []byte(fmt.Sprintf(`
			{ 
				"metric": {
					"service_id": "%s",
					"id": "%s",
					"name": "%s"
				}
			}`, serviceID, metric.id, metric.name))
		url := InternalURL + "/services/" + serviceID + "/metrics/" + metric.id
		res, err := executeHTTPRequest(http.MethodPost, url, &headerData)
		if err != nil {
			return err
		}
		if res.StatusCode != 201 {
			return fmt.Errorf("Failed to add a metric to the service(id: %s)", serviceID)
		}
	}
	return nil
}

// DeleteMetrics deletes metrics
func DeleteMetrics(serviceID string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		url := InternalURL + "/services/" + serviceID + "/metrics/" + metric.id
		res, err := executeHTTPRequest(http.MethodDelete, url, nil)
		if err != nil {
			return err
		}
		if res.StatusCode != 200 {
			return fmt.Errorf("Failed to delete the metric(service_id: %s, metric: %s)", serviceID, metric.name)
		}
	}
	return nil
}

// UpdateUsageLimit updates usage limit.
func UpdateUsageLimit(serviceID string, planID string, metricID string, limit UsageLimit) error {
	headerData := []byte(fmt.Sprintf(`
		{ 
			"usagelimit": {
				"%s": "%d"
			}
		}`, limit.period.String(), limit.value))
	url := InternalURL + "/services/" + serviceID + "/plans/" + planID + "/usagelimits/" + metricID + "/" + limit.period.String()
	res, err := executeHTTPRequest(http.MethodPut, url, &headerData)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return fmt.Errorf("Failed to update usage limits for a metric(id: %s)", metricID)
	}
	return nil
}

// UpdateUsageLimits updates usage limits.
func UpdateUsageLimits(serviceID string, planID string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		for _, limit := range metric.limits {
			if err := UpdateUsageLimit(serviceID, planID, metric.id, limit); err != nil {
				return err
			}
		}
	}
	return nil
}

// DeleteUsageLimit already-set usage limit.
func DeleteUsageLimit(serviceID string, planID string, metricID string, period Period) error {
	url := InternalURL + "/services/" + serviceID + "/plans/" + planID + "/usagelimits/" + metricID + "/" + period.String()
	res, err := executeHTTPRequest(http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	if res.StatusCode != 200 {
		return errors.New("Failed to delete usage limits for a metric")
	}
	return nil
}

// DeleteUsageLimits is a wrapper function for DeleteUsageLimit to delete multiple limits at once.
func DeleteUsageLimits(serviceID string, planID string, metrics *[]Metric) error {
	for _, metric := range *metrics {
		for _, limit := range metric.limits {
			if err := DeleteUsageLimit(serviceID, planID, metric.id, limit.period); err != nil {
				return err
			}
		}
	}
	return nil
}

func executeHTTPRequest(method string, url string, data *[]byte) (*http.Response, error) {
	client := &http.Client{}
	var body io.Reader
	if data != nil {
		body = bytes.NewBuffer(*data)
	}
	req, err := http.NewRequest(method, url, body)
	if err != nil {
		fmt.Printf("Error while creating HTTP request: %v", err)
		return nil, err
	}
	res, err := client.Do(req)
	if err != nil {
		fmt.Printf("Error sending the HTTP request: %v", err)
		return nil, err
	}
	defer res.Body.Close()
	return res, nil
}
