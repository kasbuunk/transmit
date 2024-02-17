package transmit

import (
	"context"
	"testing"
	"time"

	"github.com/google/uuid"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func TestScheduleTransmission(t *testing.T) {
	ctx := context.Background()

	// Test assumes the Transmit service is running and accessible on the following host:
	transmitServiceHost := "localhost:8080"

	grpcConnection, err := grpc.DialContext(ctx, transmitServiceHost, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Errorf("failed to initiate grpc connection: %v", err)
		t.FailNow()
	}

	scheduleTransmissionRequest := ScheduleTransmissionRequest{
		Schedule: &ScheduleTransmissionRequest_Delayed{
			Delayed: &Delayed{
				TransmitAt: timestamppb.New(time.Now().Add(time.Second)),
			},
		},
		Message: &ScheduleTransmissionRequest_NatsEvent{
			NatsEvent: &NatsEvent{
				Subject: "INTEGRATION.go_test",
				Payload: []byte("Payload of the message."),
			},
		},
	}
	grpcClient := NewTransmitClient(grpcConnection)

	response, err := grpcClient.ScheduleTransmission(ctx, &scheduleTransmissionRequest)
	if err != nil {
		t.Errorf("failed to schedule transmission over grpc: %v", err)
		t.FailNow()
	}

	if response == nil {
		t.Error("grpc response was nil, but no error was returned")
		t.FailNow()
	}

	transmissionID := response.GetTransmissionId()
	err = uuid.Validate(transmissionID)
	if err != nil {
		t.Errorf("failed to parse response id %s as uuid: %v", transmissionID, err)
	}
}
