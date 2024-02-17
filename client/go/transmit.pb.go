// Code generated by protoc-gen-go. DO NOT EDIT.
// versions:
// 	protoc-gen-go v1.28.1
// 	protoc        v4.25.3
// source: transmit.proto

package transmit

import (
	protoreflect "google.golang.org/protobuf/reflect/protoreflect"
	protoimpl "google.golang.org/protobuf/runtime/protoimpl"
	durationpb "google.golang.org/protobuf/types/known/durationpb"
	timestamppb "google.golang.org/protobuf/types/known/timestamppb"
	reflect "reflect"
	sync "sync"
)

const (
	// Verify that this generated code is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(20 - protoimpl.MinVersion)
	// Verify that runtime/protoimpl is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(protoimpl.MaxVersion - 20)
)

type HealthCheckResponse_ServingStatus int32

const (
	HealthCheckResponse_UNKNOWN         HealthCheckResponse_ServingStatus = 0
	HealthCheckResponse_SERVING         HealthCheckResponse_ServingStatus = 1
	HealthCheckResponse_NOT_SERVING     HealthCheckResponse_ServingStatus = 2
	HealthCheckResponse_SERVICE_UNKNOWN HealthCheckResponse_ServingStatus = 3 // Used only by the Watch method.
)

// Enum value maps for HealthCheckResponse_ServingStatus.
var (
	HealthCheckResponse_ServingStatus_name = map[int32]string{
		0: "UNKNOWN",
		1: "SERVING",
		2: "NOT_SERVING",
		3: "SERVICE_UNKNOWN",
	}
	HealthCheckResponse_ServingStatus_value = map[string]int32{
		"UNKNOWN":         0,
		"SERVING":         1,
		"NOT_SERVING":     2,
		"SERVICE_UNKNOWN": 3,
	}
)

func (x HealthCheckResponse_ServingStatus) Enum() *HealthCheckResponse_ServingStatus {
	p := new(HealthCheckResponse_ServingStatus)
	*p = x
	return p
}

func (x HealthCheckResponse_ServingStatus) String() string {
	return protoimpl.X.EnumStringOf(x.Descriptor(), protoreflect.EnumNumber(x))
}

func (HealthCheckResponse_ServingStatus) Descriptor() protoreflect.EnumDescriptor {
	return file_transmit_proto_enumTypes[0].Descriptor()
}

func (HealthCheckResponse_ServingStatus) Type() protoreflect.EnumType {
	return &file_transmit_proto_enumTypes[0]
}

func (x HealthCheckResponse_ServingStatus) Number() protoreflect.EnumNumber {
	return protoreflect.EnumNumber(x)
}

// Deprecated: Use HealthCheckResponse_ServingStatus.Descriptor instead.
func (HealthCheckResponse_ServingStatus) EnumDescriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{7, 0}
}

type ScheduleTransmissionRequest struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	// Types that are assignable to Schedule:
	//
	//	*ScheduleTransmissionRequest_Delayed
	//	*ScheduleTransmissionRequest_Interval
	//	*ScheduleTransmissionRequest_Cron
	Schedule isScheduleTransmissionRequest_Schedule `protobuf_oneof:"Schedule"`
	// Types that are assignable to Message:
	//
	//	*ScheduleTransmissionRequest_NatsEvent
	Message isScheduleTransmissionRequest_Message `protobuf_oneof:"Message"`
}

func (x *ScheduleTransmissionRequest) Reset() {
	*x = ScheduleTransmissionRequest{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[0]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *ScheduleTransmissionRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*ScheduleTransmissionRequest) ProtoMessage() {}

func (x *ScheduleTransmissionRequest) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[0]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use ScheduleTransmissionRequest.ProtoReflect.Descriptor instead.
func (*ScheduleTransmissionRequest) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{0}
}

func (m *ScheduleTransmissionRequest) GetSchedule() isScheduleTransmissionRequest_Schedule {
	if m != nil {
		return m.Schedule
	}
	return nil
}

func (x *ScheduleTransmissionRequest) GetDelayed() *Delayed {
	if x, ok := x.GetSchedule().(*ScheduleTransmissionRequest_Delayed); ok {
		return x.Delayed
	}
	return nil
}

func (x *ScheduleTransmissionRequest) GetInterval() *Interval {
	if x, ok := x.GetSchedule().(*ScheduleTransmissionRequest_Interval); ok {
		return x.Interval
	}
	return nil
}

func (x *ScheduleTransmissionRequest) GetCron() *Cron {
	if x, ok := x.GetSchedule().(*ScheduleTransmissionRequest_Cron); ok {
		return x.Cron
	}
	return nil
}

func (m *ScheduleTransmissionRequest) GetMessage() isScheduleTransmissionRequest_Message {
	if m != nil {
		return m.Message
	}
	return nil
}

func (x *ScheduleTransmissionRequest) GetNatsEvent() *NatsEvent {
	if x, ok := x.GetMessage().(*ScheduleTransmissionRequest_NatsEvent); ok {
		return x.NatsEvent
	}
	return nil
}

type isScheduleTransmissionRequest_Schedule interface {
	isScheduleTransmissionRequest_Schedule()
}

type ScheduleTransmissionRequest_Delayed struct {
	Delayed *Delayed `protobuf:"bytes,1,opt,name=delayed,proto3,oneof"`
}

type ScheduleTransmissionRequest_Interval struct {
	Interval *Interval `protobuf:"bytes,2,opt,name=interval,proto3,oneof"`
}

type ScheduleTransmissionRequest_Cron struct {
	Cron *Cron `protobuf:"bytes,3,opt,name=cron,proto3,oneof"`
}

func (*ScheduleTransmissionRequest_Delayed) isScheduleTransmissionRequest_Schedule() {}

func (*ScheduleTransmissionRequest_Interval) isScheduleTransmissionRequest_Schedule() {}

func (*ScheduleTransmissionRequest_Cron) isScheduleTransmissionRequest_Schedule() {}

type isScheduleTransmissionRequest_Message interface {
	isScheduleTransmissionRequest_Message()
}

type ScheduleTransmissionRequest_NatsEvent struct {
	NatsEvent *NatsEvent `protobuf:"bytes,4,opt,name=nats_event,json=natsEvent,proto3,oneof"`
}

func (*ScheduleTransmissionRequest_NatsEvent) isScheduleTransmissionRequest_Message() {}

type ScheduleTransmissionResponse struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	TransmissionId string `protobuf:"bytes,1,opt,name=transmission_id,json=transmissionId,proto3" json:"transmission_id,omitempty"`
}

func (x *ScheduleTransmissionResponse) Reset() {
	*x = ScheduleTransmissionResponse{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[1]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *ScheduleTransmissionResponse) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*ScheduleTransmissionResponse) ProtoMessage() {}

func (x *ScheduleTransmissionResponse) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[1]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use ScheduleTransmissionResponse.ProtoReflect.Descriptor instead.
func (*ScheduleTransmissionResponse) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{1}
}

func (x *ScheduleTransmissionResponse) GetTransmissionId() string {
	if x != nil {
		return x.TransmissionId
	}
	return ""
}

type NatsEvent struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	Subject string `protobuf:"bytes,1,opt,name=subject,proto3" json:"subject,omitempty"`
	Payload []byte `protobuf:"bytes,2,opt,name=payload,proto3" json:"payload,omitempty"`
}

func (x *NatsEvent) Reset() {
	*x = NatsEvent{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[2]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *NatsEvent) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*NatsEvent) ProtoMessage() {}

func (x *NatsEvent) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[2]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use NatsEvent.ProtoReflect.Descriptor instead.
func (*NatsEvent) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{2}
}

func (x *NatsEvent) GetSubject() string {
	if x != nil {
		return x.Subject
	}
	return ""
}

func (x *NatsEvent) GetPayload() []byte {
	if x != nil {
		return x.Payload
	}
	return nil
}

type Delayed struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	TransmitAt *timestamppb.Timestamp `protobuf:"bytes,1,opt,name=transmit_at,json=transmitAt,proto3" json:"transmit_at,omitempty"`
}

func (x *Delayed) Reset() {
	*x = Delayed{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[3]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *Delayed) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Delayed) ProtoMessage() {}

func (x *Delayed) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[3]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use Delayed.ProtoReflect.Descriptor instead.
func (*Delayed) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{3}
}

func (x *Delayed) GetTransmitAt() *timestamppb.Timestamp {
	if x != nil {
		return x.TransmitAt
	}
	return nil
}

type Interval struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	FirstTransmission *timestamppb.Timestamp `protobuf:"bytes,1,opt,name=first_transmission,json=firstTransmission,proto3" json:"first_transmission,omitempty"`
	Interval          *durationpb.Duration   `protobuf:"bytes,2,opt,name=interval,proto3" json:"interval,omitempty"`
	// Types that are assignable to Iterate:
	//
	//	*Interval_Times
	//	*Interval_Infinitely
	Iterate isInterval_Iterate `protobuf_oneof:"Iterate"`
}

func (x *Interval) Reset() {
	*x = Interval{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[4]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *Interval) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Interval) ProtoMessage() {}

func (x *Interval) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[4]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use Interval.ProtoReflect.Descriptor instead.
func (*Interval) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{4}
}

func (x *Interval) GetFirstTransmission() *timestamppb.Timestamp {
	if x != nil {
		return x.FirstTransmission
	}
	return nil
}

func (x *Interval) GetInterval() *durationpb.Duration {
	if x != nil {
		return x.Interval
	}
	return nil
}

func (m *Interval) GetIterate() isInterval_Iterate {
	if m != nil {
		return m.Iterate
	}
	return nil
}

func (x *Interval) GetTimes() uint32 {
	if x, ok := x.GetIterate().(*Interval_Times); ok {
		return x.Times
	}
	return 0
}

func (x *Interval) GetInfinitely() bool {
	if x, ok := x.GetIterate().(*Interval_Infinitely); ok {
		return x.Infinitely
	}
	return false
}

type isInterval_Iterate interface {
	isInterval_Iterate()
}

type Interval_Times struct {
	Times uint32 `protobuf:"varint,3,opt,name=times,proto3,oneof"`
}

type Interval_Infinitely struct {
	Infinitely bool `protobuf:"varint,4,opt,name=infinitely,proto3,oneof"`
}

func (*Interval_Times) isInterval_Iterate() {}

func (*Interval_Infinitely) isInterval_Iterate() {}

type Cron struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	FirstTransmissionAfter *timestamppb.Timestamp `protobuf:"bytes,1,opt,name=first_transmission_after,json=firstTransmissionAfter,proto3" json:"first_transmission_after,omitempty"`
	Schedule               string                 `protobuf:"bytes,2,opt,name=schedule,proto3" json:"schedule,omitempty"`
	// Types that are assignable to Iterate:
	//
	//	*Cron_Times
	//	*Cron_Infinitely
	Iterate isCron_Iterate `protobuf_oneof:"Iterate"`
}

func (x *Cron) Reset() {
	*x = Cron{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[5]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *Cron) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*Cron) ProtoMessage() {}

func (x *Cron) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[5]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use Cron.ProtoReflect.Descriptor instead.
func (*Cron) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{5}
}

func (x *Cron) GetFirstTransmissionAfter() *timestamppb.Timestamp {
	if x != nil {
		return x.FirstTransmissionAfter
	}
	return nil
}

func (x *Cron) GetSchedule() string {
	if x != nil {
		return x.Schedule
	}
	return ""
}

func (m *Cron) GetIterate() isCron_Iterate {
	if m != nil {
		return m.Iterate
	}
	return nil
}

func (x *Cron) GetTimes() uint32 {
	if x, ok := x.GetIterate().(*Cron_Times); ok {
		return x.Times
	}
	return 0
}

func (x *Cron) GetInfinitely() bool {
	if x, ok := x.GetIterate().(*Cron_Infinitely); ok {
		return x.Infinitely
	}
	return false
}

type isCron_Iterate interface {
	isCron_Iterate()
}

type Cron_Times struct {
	Times uint32 `protobuf:"varint,3,opt,name=times,proto3,oneof"`
}

type Cron_Infinitely struct {
	Infinitely bool `protobuf:"varint,4,opt,name=infinitely,proto3,oneof"`
}

func (*Cron_Times) isCron_Iterate() {}

func (*Cron_Infinitely) isCron_Iterate() {}

type HealthCheckRequest struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	Service string `protobuf:"bytes,1,opt,name=service,proto3" json:"service,omitempty"`
}

func (x *HealthCheckRequest) Reset() {
	*x = HealthCheckRequest{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[6]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *HealthCheckRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*HealthCheckRequest) ProtoMessage() {}

func (x *HealthCheckRequest) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[6]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use HealthCheckRequest.ProtoReflect.Descriptor instead.
func (*HealthCheckRequest) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{6}
}

func (x *HealthCheckRequest) GetService() string {
	if x != nil {
		return x.Service
	}
	return ""
}

type HealthCheckResponse struct {
	state         protoimpl.MessageState
	sizeCache     protoimpl.SizeCache
	unknownFields protoimpl.UnknownFields

	Status HealthCheckResponse_ServingStatus `protobuf:"varint,1,opt,name=status,proto3,enum=transmit.HealthCheckResponse_ServingStatus" json:"status,omitempty"`
}

func (x *HealthCheckResponse) Reset() {
	*x = HealthCheckResponse{}
	if protoimpl.UnsafeEnabled {
		mi := &file_transmit_proto_msgTypes[7]
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		ms.StoreMessageInfo(mi)
	}
}

func (x *HealthCheckResponse) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*HealthCheckResponse) ProtoMessage() {}

func (x *HealthCheckResponse) ProtoReflect() protoreflect.Message {
	mi := &file_transmit_proto_msgTypes[7]
	if protoimpl.UnsafeEnabled && x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use HealthCheckResponse.ProtoReflect.Descriptor instead.
func (*HealthCheckResponse) Descriptor() ([]byte, []int) {
	return file_transmit_proto_rawDescGZIP(), []int{7}
}

func (x *HealthCheckResponse) GetStatus() HealthCheckResponse_ServingStatus {
	if x != nil {
		return x.Status
	}
	return HealthCheckResponse_UNKNOWN
}

var File_transmit_proto protoreflect.FileDescriptor

var file_transmit_proto_rawDesc = []byte{
	0x0a, 0x0e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f,
	0x12, 0x08, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x1a, 0x1f, 0x67, 0x6f, 0x6f, 0x67,
	0x6c, 0x65, 0x2f, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62, 0x75, 0x66, 0x2f, 0x74, 0x69, 0x6d, 0x65,
	0x73, 0x74, 0x61, 0x6d, 0x70, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x1a, 0x1e, 0x67, 0x6f, 0x6f,
	0x67, 0x6c, 0x65, 0x2f, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62, 0x75, 0x66, 0x2f, 0x64, 0x75, 0x72,
	0x61, 0x74, 0x69, 0x6f, 0x6e, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x22, 0xf1, 0x01, 0x0a, 0x1b,
	0x53, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65, 0x54, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73,
	0x73, 0x69, 0x6f, 0x6e, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x12, 0x2d, 0x0a, 0x07, 0x64,
	0x65, 0x6c, 0x61, 0x79, 0x65, 0x64, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x11, 0x2e, 0x74,
	0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x44, 0x65, 0x6c, 0x61, 0x79, 0x65, 0x64, 0x48,
	0x00, 0x52, 0x07, 0x64, 0x65, 0x6c, 0x61, 0x79, 0x65, 0x64, 0x12, 0x30, 0x0a, 0x08, 0x69, 0x6e,
	0x74, 0x65, 0x72, 0x76, 0x61, 0x6c, 0x18, 0x02, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x12, 0x2e, 0x74,
	0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x49, 0x6e, 0x74, 0x65, 0x72, 0x76, 0x61, 0x6c,
	0x48, 0x00, 0x52, 0x08, 0x69, 0x6e, 0x74, 0x65, 0x72, 0x76, 0x61, 0x6c, 0x12, 0x24, 0x0a, 0x04,
	0x63, 0x72, 0x6f, 0x6e, 0x18, 0x03, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x0e, 0x2e, 0x74, 0x72, 0x61,
	0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x43, 0x72, 0x6f, 0x6e, 0x48, 0x00, 0x52, 0x04, 0x63, 0x72,
	0x6f, 0x6e, 0x12, 0x34, 0x0a, 0x0a, 0x6e, 0x61, 0x74, 0x73, 0x5f, 0x65, 0x76, 0x65, 0x6e, 0x74,
	0x18, 0x04, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x13, 0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69,
	0x74, 0x2e, 0x4e, 0x61, 0x74, 0x73, 0x45, 0x76, 0x65, 0x6e, 0x74, 0x48, 0x01, 0x52, 0x09, 0x6e,
	0x61, 0x74, 0x73, 0x45, 0x76, 0x65, 0x6e, 0x74, 0x42, 0x0a, 0x0a, 0x08, 0x53, 0x63, 0x68, 0x65,
	0x64, 0x75, 0x6c, 0x65, 0x42, 0x09, 0x0a, 0x07, 0x4d, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x22,
	0x47, 0x0a, 0x1c, 0x53, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65, 0x54, 0x72, 0x61, 0x6e, 0x73,
	0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12,
	0x27, 0x0a, 0x0f, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x5f,
	0x69, 0x64, 0x18, 0x01, 0x20, 0x01, 0x28, 0x09, 0x52, 0x0e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d,
	0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x49, 0x64, 0x22, 0x3f, 0x0a, 0x09, 0x4e, 0x61, 0x74, 0x73,
	0x45, 0x76, 0x65, 0x6e, 0x74, 0x12, 0x18, 0x0a, 0x07, 0x73, 0x75, 0x62, 0x6a, 0x65, 0x63, 0x74,
	0x18, 0x01, 0x20, 0x01, 0x28, 0x09, 0x52, 0x07, 0x73, 0x75, 0x62, 0x6a, 0x65, 0x63, 0x74, 0x12,
	0x18, 0x0a, 0x07, 0x70, 0x61, 0x79, 0x6c, 0x6f, 0x61, 0x64, 0x18, 0x02, 0x20, 0x01, 0x28, 0x0c,
	0x52, 0x07, 0x70, 0x61, 0x79, 0x6c, 0x6f, 0x61, 0x64, 0x22, 0x46, 0x0a, 0x07, 0x44, 0x65, 0x6c,
	0x61, 0x79, 0x65, 0x64, 0x12, 0x3b, 0x0a, 0x0b, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74,
	0x5f, 0x61, 0x74, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x1a, 0x2e, 0x67, 0x6f, 0x6f, 0x67,
	0x6c, 0x65, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62, 0x75, 0x66, 0x2e, 0x54, 0x69, 0x6d, 0x65,
	0x73, 0x74, 0x61, 0x6d, 0x70, 0x52, 0x0a, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x41,
	0x74, 0x22, 0xd1, 0x01, 0x0a, 0x08, 0x49, 0x6e, 0x74, 0x65, 0x72, 0x76, 0x61, 0x6c, 0x12, 0x49,
	0x0a, 0x12, 0x66, 0x69, 0x72, 0x73, 0x74, 0x5f, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73,
	0x73, 0x69, 0x6f, 0x6e, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x1a, 0x2e, 0x67, 0x6f, 0x6f,
	0x67, 0x6c, 0x65, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62, 0x75, 0x66, 0x2e, 0x54, 0x69, 0x6d,
	0x65, 0x73, 0x74, 0x61, 0x6d, 0x70, 0x52, 0x11, 0x66, 0x69, 0x72, 0x73, 0x74, 0x54, 0x72, 0x61,
	0x6e, 0x73, 0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x12, 0x35, 0x0a, 0x08, 0x69, 0x6e, 0x74,
	0x65, 0x72, 0x76, 0x61, 0x6c, 0x18, 0x02, 0x20, 0x01, 0x28, 0x0b, 0x32, 0x19, 0x2e, 0x67, 0x6f,
	0x6f, 0x67, 0x6c, 0x65, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62, 0x75, 0x66, 0x2e, 0x44, 0x75,
	0x72, 0x61, 0x74, 0x69, 0x6f, 0x6e, 0x52, 0x08, 0x69, 0x6e, 0x74, 0x65, 0x72, 0x76, 0x61, 0x6c,
	0x12, 0x16, 0x0a, 0x05, 0x74, 0x69, 0x6d, 0x65, 0x73, 0x18, 0x03, 0x20, 0x01, 0x28, 0x0d, 0x48,
	0x00, 0x52, 0x05, 0x74, 0x69, 0x6d, 0x65, 0x73, 0x12, 0x20, 0x0a, 0x0a, 0x69, 0x6e, 0x66, 0x69,
	0x6e, 0x69, 0x74, 0x65, 0x6c, 0x79, 0x18, 0x04, 0x20, 0x01, 0x28, 0x08, 0x48, 0x00, 0x52, 0x0a,
	0x69, 0x6e, 0x66, 0x69, 0x6e, 0x69, 0x74, 0x65, 0x6c, 0x79, 0x42, 0x09, 0x0a, 0x07, 0x49, 0x74,
	0x65, 0x72, 0x61, 0x74, 0x65, 0x22, 0xbd, 0x01, 0x0a, 0x04, 0x43, 0x72, 0x6f, 0x6e, 0x12, 0x54,
	0x0a, 0x18, 0x66, 0x69, 0x72, 0x73, 0x74, 0x5f, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73,
	0x73, 0x69, 0x6f, 0x6e, 0x5f, 0x61, 0x66, 0x74, 0x65, 0x72, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0b,
	0x32, 0x1a, 0x2e, 0x67, 0x6f, 0x6f, 0x67, 0x6c, 0x65, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x62,
	0x75, 0x66, 0x2e, 0x54, 0x69, 0x6d, 0x65, 0x73, 0x74, 0x61, 0x6d, 0x70, 0x52, 0x16, 0x66, 0x69,
	0x72, 0x73, 0x74, 0x54, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x41,
	0x66, 0x74, 0x65, 0x72, 0x12, 0x1a, 0x0a, 0x08, 0x73, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65,
	0x18, 0x02, 0x20, 0x01, 0x28, 0x09, 0x52, 0x08, 0x73, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65,
	0x12, 0x16, 0x0a, 0x05, 0x74, 0x69, 0x6d, 0x65, 0x73, 0x18, 0x03, 0x20, 0x01, 0x28, 0x0d, 0x48,
	0x00, 0x52, 0x05, 0x74, 0x69, 0x6d, 0x65, 0x73, 0x12, 0x20, 0x0a, 0x0a, 0x69, 0x6e, 0x66, 0x69,
	0x6e, 0x69, 0x74, 0x65, 0x6c, 0x79, 0x18, 0x04, 0x20, 0x01, 0x28, 0x08, 0x48, 0x00, 0x52, 0x0a,
	0x69, 0x6e, 0x66, 0x69, 0x6e, 0x69, 0x74, 0x65, 0x6c, 0x79, 0x42, 0x09, 0x0a, 0x07, 0x49, 0x74,
	0x65, 0x72, 0x61, 0x74, 0x65, 0x22, 0x2e, 0x0a, 0x12, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68, 0x43,
	0x68, 0x65, 0x63, 0x6b, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x12, 0x18, 0x0a, 0x07, 0x73,
	0x65, 0x72, 0x76, 0x69, 0x63, 0x65, 0x18, 0x01, 0x20, 0x01, 0x28, 0x09, 0x52, 0x07, 0x73, 0x65,
	0x72, 0x76, 0x69, 0x63, 0x65, 0x22, 0xab, 0x01, 0x0a, 0x13, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68,
	0x43, 0x68, 0x65, 0x63, 0x6b, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12, 0x43, 0x0a,
	0x06, 0x73, 0x74, 0x61, 0x74, 0x75, 0x73, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0e, 0x32, 0x2b, 0x2e,
	0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68, 0x43,
	0x68, 0x65, 0x63, 0x6b, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x2e, 0x53, 0x65, 0x72,
	0x76, 0x69, 0x6e, 0x67, 0x53, 0x74, 0x61, 0x74, 0x75, 0x73, 0x52, 0x06, 0x73, 0x74, 0x61, 0x74,
	0x75, 0x73, 0x22, 0x4f, 0x0a, 0x0d, 0x53, 0x65, 0x72, 0x76, 0x69, 0x6e, 0x67, 0x53, 0x74, 0x61,
	0x74, 0x75, 0x73, 0x12, 0x0b, 0x0a, 0x07, 0x55, 0x4e, 0x4b, 0x4e, 0x4f, 0x57, 0x4e, 0x10, 0x00,
	0x12, 0x0b, 0x0a, 0x07, 0x53, 0x45, 0x52, 0x56, 0x49, 0x4e, 0x47, 0x10, 0x01, 0x12, 0x0f, 0x0a,
	0x0b, 0x4e, 0x4f, 0x54, 0x5f, 0x53, 0x45, 0x52, 0x56, 0x49, 0x4e, 0x47, 0x10, 0x02, 0x12, 0x13,
	0x0a, 0x0f, 0x53, 0x45, 0x52, 0x56, 0x49, 0x43, 0x45, 0x5f, 0x55, 0x4e, 0x4b, 0x4e, 0x4f, 0x57,
	0x4e, 0x10, 0x03, 0x32, 0x71, 0x0a, 0x08, 0x54, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x12,
	0x65, 0x0a, 0x14, 0x53, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65, 0x54, 0x72, 0x61, 0x6e, 0x73,
	0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x12, 0x25, 0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d,
	0x69, 0x74, 0x2e, 0x53, 0x63, 0x68, 0x65, 0x64, 0x75, 0x6c, 0x65, 0x54, 0x72, 0x61, 0x6e, 0x73,
	0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x1a, 0x26,
	0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x53, 0x63, 0x68, 0x65, 0x64, 0x75,
	0x6c, 0x65, 0x54, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x73, 0x73, 0x69, 0x6f, 0x6e, 0x52, 0x65,
	0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x32, 0x96, 0x01, 0x0a, 0x06, 0x48, 0x65, 0x61, 0x6c, 0x74,
	0x68, 0x12, 0x44, 0x0a, 0x05, 0x43, 0x68, 0x65, 0x63, 0x6b, 0x12, 0x1c, 0x2e, 0x74, 0x72, 0x61,
	0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68, 0x43, 0x68, 0x65, 0x63,
	0x6b, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x1a, 0x1d, 0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73,
	0x6d, 0x69, 0x74, 0x2e, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68, 0x43, 0x68, 0x65, 0x63, 0x6b, 0x52,
	0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12, 0x46, 0x0a, 0x05, 0x57, 0x61, 0x74, 0x63, 0x68,
	0x12, 0x1c, 0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x48, 0x65, 0x61, 0x6c,
	0x74, 0x68, 0x43, 0x68, 0x65, 0x63, 0x6b, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x1a, 0x1d,
	0x2e, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x6d, 0x69, 0x74, 0x2e, 0x48, 0x65, 0x61, 0x6c, 0x74, 0x68,
	0x43, 0x68, 0x65, 0x63, 0x6b, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x30, 0x01, 0x62,
	0x06, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x33,
}

var (
	file_transmit_proto_rawDescOnce sync.Once
	file_transmit_proto_rawDescData = file_transmit_proto_rawDesc
)

func file_transmit_proto_rawDescGZIP() []byte {
	file_transmit_proto_rawDescOnce.Do(func() {
		file_transmit_proto_rawDescData = protoimpl.X.CompressGZIP(file_transmit_proto_rawDescData)
	})
	return file_transmit_proto_rawDescData
}

var file_transmit_proto_enumTypes = make([]protoimpl.EnumInfo, 1)
var file_transmit_proto_msgTypes = make([]protoimpl.MessageInfo, 8)
var file_transmit_proto_goTypes = []interface{}{
	(HealthCheckResponse_ServingStatus)(0), // 0: transmit.HealthCheckResponse.ServingStatus
	(*ScheduleTransmissionRequest)(nil),    // 1: transmit.ScheduleTransmissionRequest
	(*ScheduleTransmissionResponse)(nil),   // 2: transmit.ScheduleTransmissionResponse
	(*NatsEvent)(nil),                      // 3: transmit.NatsEvent
	(*Delayed)(nil),                        // 4: transmit.Delayed
	(*Interval)(nil),                       // 5: transmit.Interval
	(*Cron)(nil),                           // 6: transmit.Cron
	(*HealthCheckRequest)(nil),             // 7: transmit.HealthCheckRequest
	(*HealthCheckResponse)(nil),            // 8: transmit.HealthCheckResponse
	(*timestamppb.Timestamp)(nil),          // 9: google.protobuf.Timestamp
	(*durationpb.Duration)(nil),            // 10: google.protobuf.Duration
}
var file_transmit_proto_depIdxs = []int32{
	4,  // 0: transmit.ScheduleTransmissionRequest.delayed:type_name -> transmit.Delayed
	5,  // 1: transmit.ScheduleTransmissionRequest.interval:type_name -> transmit.Interval
	6,  // 2: transmit.ScheduleTransmissionRequest.cron:type_name -> transmit.Cron
	3,  // 3: transmit.ScheduleTransmissionRequest.nats_event:type_name -> transmit.NatsEvent
	9,  // 4: transmit.Delayed.transmit_at:type_name -> google.protobuf.Timestamp
	9,  // 5: transmit.Interval.first_transmission:type_name -> google.protobuf.Timestamp
	10, // 6: transmit.Interval.interval:type_name -> google.protobuf.Duration
	9,  // 7: transmit.Cron.first_transmission_after:type_name -> google.protobuf.Timestamp
	0,  // 8: transmit.HealthCheckResponse.status:type_name -> transmit.HealthCheckResponse.ServingStatus
	1,  // 9: transmit.Transmit.ScheduleTransmission:input_type -> transmit.ScheduleTransmissionRequest
	7,  // 10: transmit.Health.Check:input_type -> transmit.HealthCheckRequest
	7,  // 11: transmit.Health.Watch:input_type -> transmit.HealthCheckRequest
	2,  // 12: transmit.Transmit.ScheduleTransmission:output_type -> transmit.ScheduleTransmissionResponse
	8,  // 13: transmit.Health.Check:output_type -> transmit.HealthCheckResponse
	8,  // 14: transmit.Health.Watch:output_type -> transmit.HealthCheckResponse
	12, // [12:15] is the sub-list for method output_type
	9,  // [9:12] is the sub-list for method input_type
	9,  // [9:9] is the sub-list for extension type_name
	9,  // [9:9] is the sub-list for extension extendee
	0,  // [0:9] is the sub-list for field type_name
}

func init() { file_transmit_proto_init() }
func file_transmit_proto_init() {
	if File_transmit_proto != nil {
		return
	}
	if !protoimpl.UnsafeEnabled {
		file_transmit_proto_msgTypes[0].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*ScheduleTransmissionRequest); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[1].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*ScheduleTransmissionResponse); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[2].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*NatsEvent); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[3].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*Delayed); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[4].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*Interval); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[5].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*Cron); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[6].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*HealthCheckRequest); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
		file_transmit_proto_msgTypes[7].Exporter = func(v interface{}, i int) interface{} {
			switch v := v.(*HealthCheckResponse); i {
			case 0:
				return &v.state
			case 1:
				return &v.sizeCache
			case 2:
				return &v.unknownFields
			default:
				return nil
			}
		}
	}
	file_transmit_proto_msgTypes[0].OneofWrappers = []interface{}{
		(*ScheduleTransmissionRequest_Delayed)(nil),
		(*ScheduleTransmissionRequest_Interval)(nil),
		(*ScheduleTransmissionRequest_Cron)(nil),
		(*ScheduleTransmissionRequest_NatsEvent)(nil),
	}
	file_transmit_proto_msgTypes[4].OneofWrappers = []interface{}{
		(*Interval_Times)(nil),
		(*Interval_Infinitely)(nil),
	}
	file_transmit_proto_msgTypes[5].OneofWrappers = []interface{}{
		(*Cron_Times)(nil),
		(*Cron_Infinitely)(nil),
	}
	type x struct{}
	out := protoimpl.TypeBuilder{
		File: protoimpl.DescBuilder{
			GoPackagePath: reflect.TypeOf(x{}).PkgPath(),
			RawDescriptor: file_transmit_proto_rawDesc,
			NumEnums:      1,
			NumMessages:   8,
			NumExtensions: 0,
			NumServices:   2,
		},
		GoTypes:           file_transmit_proto_goTypes,
		DependencyIndexes: file_transmit_proto_depIdxs,
		EnumInfos:         file_transmit_proto_enumTypes,
		MessageInfos:      file_transmit_proto_msgTypes,
	}.Build()
	File_transmit_proto = out.File
	file_transmit_proto_rawDesc = nil
	file_transmit_proto_goTypes = nil
	file_transmit_proto_depIdxs = nil
}
