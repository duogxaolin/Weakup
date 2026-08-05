// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'app_database.dart';

// ignore_for_file: type=lint
class $JobsTable extends Jobs with TableInfo<$JobsTable, JobRow> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $JobsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
    'id',
    aliasedName,
    false,
    hasAutoIncrement: true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
    defaultConstraints: GeneratedColumn.constraintIsAlways(
      'PRIMARY KEY AUTOINCREMENT',
    ),
  );
  static const VerificationMeta _typeMeta = const VerificationMeta('type');
  @override
  late final GeneratedColumn<String> type = GeneratedColumn<String>(
    'type',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _triggerKindMeta = const VerificationMeta(
    'triggerKind',
  );
  @override
  late final GeneratedColumn<String> triggerKind = GeneratedColumn<String>(
    'trigger_kind',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _triggerMinutesMeta = const VerificationMeta(
    'triggerMinutes',
  );
  @override
  late final GeneratedColumn<int> triggerMinutes = GeneratedColumn<int>(
    'trigger_minutes',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerHourMeta = const VerificationMeta(
    'triggerHour',
  );
  @override
  late final GeneratedColumn<int> triggerHour = GeneratedColumn<int>(
    'trigger_hour',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerMinuteMeta = const VerificationMeta(
    'triggerMinute',
  );
  @override
  late final GeneratedColumn<int> triggerMinute = GeneratedColumn<int>(
    'trigger_minute',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerDateMeta = const VerificationMeta(
    'triggerDate',
  );
  @override
  late final GeneratedColumn<String> triggerDate = GeneratedColumn<String>(
    'trigger_date',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _statusMeta = const VerificationMeta('status');
  @override
  late final GeneratedColumn<String> status = GeneratedColumn<String>(
    'status',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _targetInstantUtcMeta = const VerificationMeta(
    'targetInstantUtc',
  );
  @override
  late final GeneratedColumn<DateTime> targetInstantUtc =
      GeneratedColumn<DateTime>(
        'target_instant_utc',
        aliasedName,
        true,
        type: DriftSqlType.dateTime,
        requiredDuringInsert: false,
      );
  static const VerificationMeta _createdAtMeta = const VerificationMeta(
    'createdAt',
  );
  @override
  late final GeneratedColumn<DateTime> createdAt = GeneratedColumn<DateTime>(
    'created_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _failureMessageMeta = const VerificationMeta(
    'failureMessage',
  );
  @override
  late final GeneratedColumn<String> failureMessage = GeneratedColumn<String>(
    'failure_message',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  @override
  List<GeneratedColumn> get $columns => [
    id,
    type,
    triggerKind,
    triggerMinutes,
    triggerHour,
    triggerMinute,
    triggerDate,
    status,
    targetInstantUtc,
    createdAt,
    updatedAt,
    failureMessage,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'jobs';
  @override
  VerificationContext validateIntegrity(
    Insertable<JobRow> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('type')) {
      context.handle(
        _typeMeta,
        type.isAcceptableOrUnknown(data['type']!, _typeMeta),
      );
    } else if (isInserting) {
      context.missing(_typeMeta);
    }
    if (data.containsKey('trigger_kind')) {
      context.handle(
        _triggerKindMeta,
        triggerKind.isAcceptableOrUnknown(
          data['trigger_kind']!,
          _triggerKindMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_triggerKindMeta);
    }
    if (data.containsKey('trigger_minutes')) {
      context.handle(
        _triggerMinutesMeta,
        triggerMinutes.isAcceptableOrUnknown(
          data['trigger_minutes']!,
          _triggerMinutesMeta,
        ),
      );
    }
    if (data.containsKey('trigger_hour')) {
      context.handle(
        _triggerHourMeta,
        triggerHour.isAcceptableOrUnknown(
          data['trigger_hour']!,
          _triggerHourMeta,
        ),
      );
    }
    if (data.containsKey('trigger_minute')) {
      context.handle(
        _triggerMinuteMeta,
        triggerMinute.isAcceptableOrUnknown(
          data['trigger_minute']!,
          _triggerMinuteMeta,
        ),
      );
    }
    if (data.containsKey('trigger_date')) {
      context.handle(
        _triggerDateMeta,
        triggerDate.isAcceptableOrUnknown(
          data['trigger_date']!,
          _triggerDateMeta,
        ),
      );
    }
    if (data.containsKey('status')) {
      context.handle(
        _statusMeta,
        status.isAcceptableOrUnknown(data['status']!, _statusMeta),
      );
    } else if (isInserting) {
      context.missing(_statusMeta);
    }
    if (data.containsKey('target_instant_utc')) {
      context.handle(
        _targetInstantUtcMeta,
        targetInstantUtc.isAcceptableOrUnknown(
          data['target_instant_utc']!,
          _targetInstantUtcMeta,
        ),
      );
    }
    if (data.containsKey('created_at')) {
      context.handle(
        _createdAtMeta,
        createdAt.isAcceptableOrUnknown(data['created_at']!, _createdAtMeta),
      );
    } else if (isInserting) {
      context.missing(_createdAtMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    if (data.containsKey('failure_message')) {
      context.handle(
        _failureMessageMeta,
        failureMessage.isAcceptableOrUnknown(
          data['failure_message']!,
          _failureMessageMeta,
        ),
      );
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  JobRow map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return JobRow(
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}id'],
      )!,
      type: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}type'],
      )!,
      triggerKind: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}trigger_kind'],
      )!,
      triggerMinutes: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_minutes'],
      ),
      triggerHour: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_hour'],
      ),
      triggerMinute: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_minute'],
      ),
      triggerDate: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}trigger_date'],
      ),
      status: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}status'],
      )!,
      targetInstantUtc: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}target_instant_utc'],
      ),
      createdAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}created_at'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
      failureMessage: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}failure_message'],
      ),
    );
  }

  @override
  $JobsTable createAlias(String alias) {
    return $JobsTable(attachedDatabase, alias);
  }
}

class JobRow extends DataClass implements Insertable<JobRow> {
  final int id;
  final String type;
  final String triggerKind;
  final int? triggerMinutes;
  final int? triggerHour;
  final int? triggerMinute;

  /// The exact local date an AbsoluteTimeTrigger fires on, as `YYYY-MM-DD`.
  ///
  /// Nullable, and null means "a time of day" — which is what every row written
  /// before this column existed meant, so the migration needs no backfill.
  ///
  /// Text rather than a `DateTimeColumn`: a wall-clock date is not an instant, and
  /// storing it as one would attach a zone the value must not carry.
  final String? triggerDate;
  final String status;
  final DateTime? targetInstantUtc;
  final DateTime createdAt;
  final DateTime updatedAt;
  final String? failureMessage;
  const JobRow({
    required this.id,
    required this.type,
    required this.triggerKind,
    this.triggerMinutes,
    this.triggerHour,
    this.triggerMinute,
    this.triggerDate,
    required this.status,
    this.targetInstantUtc,
    required this.createdAt,
    required this.updatedAt,
    this.failureMessage,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['type'] = Variable<String>(type);
    map['trigger_kind'] = Variable<String>(triggerKind);
    if (!nullToAbsent || triggerMinutes != null) {
      map['trigger_minutes'] = Variable<int>(triggerMinutes);
    }
    if (!nullToAbsent || triggerHour != null) {
      map['trigger_hour'] = Variable<int>(triggerHour);
    }
    if (!nullToAbsent || triggerMinute != null) {
      map['trigger_minute'] = Variable<int>(triggerMinute);
    }
    if (!nullToAbsent || triggerDate != null) {
      map['trigger_date'] = Variable<String>(triggerDate);
    }
    map['status'] = Variable<String>(status);
    if (!nullToAbsent || targetInstantUtc != null) {
      map['target_instant_utc'] = Variable<DateTime>(targetInstantUtc);
    }
    map['created_at'] = Variable<DateTime>(createdAt);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    if (!nullToAbsent || failureMessage != null) {
      map['failure_message'] = Variable<String>(failureMessage);
    }
    return map;
  }

  JobsCompanion toCompanion(bool nullToAbsent) {
    return JobsCompanion(
      id: Value(id),
      type: Value(type),
      triggerKind: Value(triggerKind),
      triggerMinutes: triggerMinutes == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerMinutes),
      triggerHour: triggerHour == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerHour),
      triggerMinute: triggerMinute == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerMinute),
      triggerDate: triggerDate == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerDate),
      status: Value(status),
      targetInstantUtc: targetInstantUtc == null && nullToAbsent
          ? const Value.absent()
          : Value(targetInstantUtc),
      createdAt: Value(createdAt),
      updatedAt: Value(updatedAt),
      failureMessage: failureMessage == null && nullToAbsent
          ? const Value.absent()
          : Value(failureMessage),
    );
  }

  factory JobRow.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return JobRow(
      id: serializer.fromJson<int>(json['id']),
      type: serializer.fromJson<String>(json['type']),
      triggerKind: serializer.fromJson<String>(json['triggerKind']),
      triggerMinutes: serializer.fromJson<int?>(json['triggerMinutes']),
      triggerHour: serializer.fromJson<int?>(json['triggerHour']),
      triggerMinute: serializer.fromJson<int?>(json['triggerMinute']),
      triggerDate: serializer.fromJson<String?>(json['triggerDate']),
      status: serializer.fromJson<String>(json['status']),
      targetInstantUtc: serializer.fromJson<DateTime?>(
        json['targetInstantUtc'],
      ),
      createdAt: serializer.fromJson<DateTime>(json['createdAt']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
      failureMessage: serializer.fromJson<String?>(json['failureMessage']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'type': serializer.toJson<String>(type),
      'triggerKind': serializer.toJson<String>(triggerKind),
      'triggerMinutes': serializer.toJson<int?>(triggerMinutes),
      'triggerHour': serializer.toJson<int?>(triggerHour),
      'triggerMinute': serializer.toJson<int?>(triggerMinute),
      'triggerDate': serializer.toJson<String?>(triggerDate),
      'status': serializer.toJson<String>(status),
      'targetInstantUtc': serializer.toJson<DateTime?>(targetInstantUtc),
      'createdAt': serializer.toJson<DateTime>(createdAt),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
      'failureMessage': serializer.toJson<String?>(failureMessage),
    };
  }

  JobRow copyWith({
    int? id,
    String? type,
    String? triggerKind,
    Value<int?> triggerMinutes = const Value.absent(),
    Value<int?> triggerHour = const Value.absent(),
    Value<int?> triggerMinute = const Value.absent(),
    Value<String?> triggerDate = const Value.absent(),
    String? status,
    Value<DateTime?> targetInstantUtc = const Value.absent(),
    DateTime? createdAt,
    DateTime? updatedAt,
    Value<String?> failureMessage = const Value.absent(),
  }) => JobRow(
    id: id ?? this.id,
    type: type ?? this.type,
    triggerKind: triggerKind ?? this.triggerKind,
    triggerMinutes: triggerMinutes.present
        ? triggerMinutes.value
        : this.triggerMinutes,
    triggerHour: triggerHour.present ? triggerHour.value : this.triggerHour,
    triggerMinute: triggerMinute.present
        ? triggerMinute.value
        : this.triggerMinute,
    triggerDate: triggerDate.present ? triggerDate.value : this.triggerDate,
    status: status ?? this.status,
    targetInstantUtc: targetInstantUtc.present
        ? targetInstantUtc.value
        : this.targetInstantUtc,
    createdAt: createdAt ?? this.createdAt,
    updatedAt: updatedAt ?? this.updatedAt,
    failureMessage: failureMessage.present
        ? failureMessage.value
        : this.failureMessage,
  );
  JobRow copyWithCompanion(JobsCompanion data) {
    return JobRow(
      id: data.id.present ? data.id.value : this.id,
      type: data.type.present ? data.type.value : this.type,
      triggerKind: data.triggerKind.present
          ? data.triggerKind.value
          : this.triggerKind,
      triggerMinutes: data.triggerMinutes.present
          ? data.triggerMinutes.value
          : this.triggerMinutes,
      triggerHour: data.triggerHour.present
          ? data.triggerHour.value
          : this.triggerHour,
      triggerMinute: data.triggerMinute.present
          ? data.triggerMinute.value
          : this.triggerMinute,
      triggerDate: data.triggerDate.present
          ? data.triggerDate.value
          : this.triggerDate,
      status: data.status.present ? data.status.value : this.status,
      targetInstantUtc: data.targetInstantUtc.present
          ? data.targetInstantUtc.value
          : this.targetInstantUtc,
      createdAt: data.createdAt.present ? data.createdAt.value : this.createdAt,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
      failureMessage: data.failureMessage.present
          ? data.failureMessage.value
          : this.failureMessage,
    );
  }

  @override
  String toString() {
    return (StringBuffer('JobRow(')
          ..write('id: $id, ')
          ..write('type: $type, ')
          ..write('triggerKind: $triggerKind, ')
          ..write('triggerMinutes: $triggerMinutes, ')
          ..write('triggerHour: $triggerHour, ')
          ..write('triggerMinute: $triggerMinute, ')
          ..write('triggerDate: $triggerDate, ')
          ..write('status: $status, ')
          ..write('targetInstantUtc: $targetInstantUtc, ')
          ..write('createdAt: $createdAt, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('failureMessage: $failureMessage')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    id,
    type,
    triggerKind,
    triggerMinutes,
    triggerHour,
    triggerMinute,
    triggerDate,
    status,
    targetInstantUtc,
    createdAt,
    updatedAt,
    failureMessage,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is JobRow &&
          other.id == this.id &&
          other.type == this.type &&
          other.triggerKind == this.triggerKind &&
          other.triggerMinutes == this.triggerMinutes &&
          other.triggerHour == this.triggerHour &&
          other.triggerMinute == this.triggerMinute &&
          other.triggerDate == this.triggerDate &&
          other.status == this.status &&
          other.targetInstantUtc == this.targetInstantUtc &&
          other.createdAt == this.createdAt &&
          other.updatedAt == this.updatedAt &&
          other.failureMessage == this.failureMessage);
}

class JobsCompanion extends UpdateCompanion<JobRow> {
  final Value<int> id;
  final Value<String> type;
  final Value<String> triggerKind;
  final Value<int?> triggerMinutes;
  final Value<int?> triggerHour;
  final Value<int?> triggerMinute;
  final Value<String?> triggerDate;
  final Value<String> status;
  final Value<DateTime?> targetInstantUtc;
  final Value<DateTime> createdAt;
  final Value<DateTime> updatedAt;
  final Value<String?> failureMessage;
  const JobsCompanion({
    this.id = const Value.absent(),
    this.type = const Value.absent(),
    this.triggerKind = const Value.absent(),
    this.triggerMinutes = const Value.absent(),
    this.triggerHour = const Value.absent(),
    this.triggerMinute = const Value.absent(),
    this.triggerDate = const Value.absent(),
    this.status = const Value.absent(),
    this.targetInstantUtc = const Value.absent(),
    this.createdAt = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.failureMessage = const Value.absent(),
  });
  JobsCompanion.insert({
    this.id = const Value.absent(),
    required String type,
    required String triggerKind,
    this.triggerMinutes = const Value.absent(),
    this.triggerHour = const Value.absent(),
    this.triggerMinute = const Value.absent(),
    this.triggerDate = const Value.absent(),
    required String status,
    this.targetInstantUtc = const Value.absent(),
    required DateTime createdAt,
    required DateTime updatedAt,
    this.failureMessage = const Value.absent(),
  }) : type = Value(type),
       triggerKind = Value(triggerKind),
       status = Value(status),
       createdAt = Value(createdAt),
       updatedAt = Value(updatedAt);
  static Insertable<JobRow> custom({
    Expression<int>? id,
    Expression<String>? type,
    Expression<String>? triggerKind,
    Expression<int>? triggerMinutes,
    Expression<int>? triggerHour,
    Expression<int>? triggerMinute,
    Expression<String>? triggerDate,
    Expression<String>? status,
    Expression<DateTime>? targetInstantUtc,
    Expression<DateTime>? createdAt,
    Expression<DateTime>? updatedAt,
    Expression<String>? failureMessage,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (type != null) 'type': type,
      if (triggerKind != null) 'trigger_kind': triggerKind,
      if (triggerMinutes != null) 'trigger_minutes': triggerMinutes,
      if (triggerHour != null) 'trigger_hour': triggerHour,
      if (triggerMinute != null) 'trigger_minute': triggerMinute,
      if (triggerDate != null) 'trigger_date': triggerDate,
      if (status != null) 'status': status,
      if (targetInstantUtc != null) 'target_instant_utc': targetInstantUtc,
      if (createdAt != null) 'created_at': createdAt,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (failureMessage != null) 'failure_message': failureMessage,
    });
  }

  JobsCompanion copyWith({
    Value<int>? id,
    Value<String>? type,
    Value<String>? triggerKind,
    Value<int?>? triggerMinutes,
    Value<int?>? triggerHour,
    Value<int?>? triggerMinute,
    Value<String?>? triggerDate,
    Value<String>? status,
    Value<DateTime?>? targetInstantUtc,
    Value<DateTime>? createdAt,
    Value<DateTime>? updatedAt,
    Value<String?>? failureMessage,
  }) {
    return JobsCompanion(
      id: id ?? this.id,
      type: type ?? this.type,
      triggerKind: triggerKind ?? this.triggerKind,
      triggerMinutes: triggerMinutes ?? this.triggerMinutes,
      triggerHour: triggerHour ?? this.triggerHour,
      triggerMinute: triggerMinute ?? this.triggerMinute,
      triggerDate: triggerDate ?? this.triggerDate,
      status: status ?? this.status,
      targetInstantUtc: targetInstantUtc ?? this.targetInstantUtc,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
      failureMessage: failureMessage ?? this.failureMessage,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (type.present) {
      map['type'] = Variable<String>(type.value);
    }
    if (triggerKind.present) {
      map['trigger_kind'] = Variable<String>(triggerKind.value);
    }
    if (triggerMinutes.present) {
      map['trigger_minutes'] = Variable<int>(triggerMinutes.value);
    }
    if (triggerHour.present) {
      map['trigger_hour'] = Variable<int>(triggerHour.value);
    }
    if (triggerMinute.present) {
      map['trigger_minute'] = Variable<int>(triggerMinute.value);
    }
    if (triggerDate.present) {
      map['trigger_date'] = Variable<String>(triggerDate.value);
    }
    if (status.present) {
      map['status'] = Variable<String>(status.value);
    }
    if (targetInstantUtc.present) {
      map['target_instant_utc'] = Variable<DateTime>(targetInstantUtc.value);
    }
    if (createdAt.present) {
      map['created_at'] = Variable<DateTime>(createdAt.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (failureMessage.present) {
      map['failure_message'] = Variable<String>(failureMessage.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('JobsCompanion(')
          ..write('id: $id, ')
          ..write('type: $type, ')
          ..write('triggerKind: $triggerKind, ')
          ..write('triggerMinutes: $triggerMinutes, ')
          ..write('triggerHour: $triggerHour, ')
          ..write('triggerMinute: $triggerMinute, ')
          ..write('triggerDate: $triggerDate, ')
          ..write('status: $status, ')
          ..write('targetInstantUtc: $targetInstantUtc, ')
          ..write('createdAt: $createdAt, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('failureMessage: $failureMessage')
          ..write(')'))
        .toString();
  }
}

abstract class _$AppDatabase extends GeneratedDatabase {
  _$AppDatabase(QueryExecutor e) : super(e);
  $AppDatabaseManager get managers => $AppDatabaseManager(this);
  late final $JobsTable jobs = $JobsTable(this);
  @override
  Iterable<TableInfo<Table, Object?>> get allTables =>
      allSchemaEntities.whereType<TableInfo<Table, Object?>>();
  @override
  List<DatabaseSchemaEntity> get allSchemaEntities => [jobs];
}

typedef $$JobsTableCreateCompanionBuilder =
    JobsCompanion Function({
      Value<int> id,
      required String type,
      required String triggerKind,
      Value<int?> triggerMinutes,
      Value<int?> triggerHour,
      Value<int?> triggerMinute,
      Value<String?> triggerDate,
      required String status,
      Value<DateTime?> targetInstantUtc,
      required DateTime createdAt,
      required DateTime updatedAt,
      Value<String?> failureMessage,
    });
typedef $$JobsTableUpdateCompanionBuilder =
    JobsCompanion Function({
      Value<int> id,
      Value<String> type,
      Value<String> triggerKind,
      Value<int?> triggerMinutes,
      Value<int?> triggerHour,
      Value<int?> triggerMinute,
      Value<String?> triggerDate,
      Value<String> status,
      Value<DateTime?> targetInstantUtc,
      Value<DateTime> createdAt,
      Value<DateTime> updatedAt,
      Value<String?> failureMessage,
    });

class $$JobsTableFilterComposer extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get type => $composableBuilder(
    column: $table.type,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => ColumnFilters(column),
  );
}

class $$JobsTableOrderingComposer extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get type => $composableBuilder(
    column: $table.type,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$JobsTableAnnotationComposer
    extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get type =>
      $composableBuilder(column: $table.type, builder: (column) => column);

  GeneratedColumn<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => column,
  );

  GeneratedColumn<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => column,
  );

  GeneratedColumn<String> get status =>
      $composableBuilder(column: $table.status, builder: (column) => column);

  GeneratedColumn<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get createdAt =>
      $composableBuilder(column: $table.createdAt, builder: (column) => column);

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);

  GeneratedColumn<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => column,
  );
}

class $$JobsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $JobsTable,
          JobRow,
          $$JobsTableFilterComposer,
          $$JobsTableOrderingComposer,
          $$JobsTableAnnotationComposer,
          $$JobsTableCreateCompanionBuilder,
          $$JobsTableUpdateCompanionBuilder,
          (JobRow, BaseReferences<_$AppDatabase, $JobsTable, JobRow>),
          JobRow,
          PrefetchHooks Function()
        > {
  $$JobsTableTableManager(_$AppDatabase db, $JobsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$JobsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$JobsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$JobsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                Value<String> type = const Value.absent(),
                Value<String> triggerKind = const Value.absent(),
                Value<int?> triggerMinutes = const Value.absent(),
                Value<int?> triggerHour = const Value.absent(),
                Value<int?> triggerMinute = const Value.absent(),
                Value<String?> triggerDate = const Value.absent(),
                Value<String> status = const Value.absent(),
                Value<DateTime?> targetInstantUtc = const Value.absent(),
                Value<DateTime> createdAt = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<String?> failureMessage = const Value.absent(),
              }) => JobsCompanion(
                id: id,
                type: type,
                triggerKind: triggerKind,
                triggerMinutes: triggerMinutes,
                triggerHour: triggerHour,
                triggerMinute: triggerMinute,
                triggerDate: triggerDate,
                status: status,
                targetInstantUtc: targetInstantUtc,
                createdAt: createdAt,
                updatedAt: updatedAt,
                failureMessage: failureMessage,
              ),
          createCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                required String type,
                required String triggerKind,
                Value<int?> triggerMinutes = const Value.absent(),
                Value<int?> triggerHour = const Value.absent(),
                Value<int?> triggerMinute = const Value.absent(),
                Value<String?> triggerDate = const Value.absent(),
                required String status,
                Value<DateTime?> targetInstantUtc = const Value.absent(),
                required DateTime createdAt,
                required DateTime updatedAt,
                Value<String?> failureMessage = const Value.absent(),
              }) => JobsCompanion.insert(
                id: id,
                type: type,
                triggerKind: triggerKind,
                triggerMinutes: triggerMinutes,
                triggerHour: triggerHour,
                triggerMinute: triggerMinute,
                triggerDate: triggerDate,
                status: status,
                targetInstantUtc: targetInstantUtc,
                createdAt: createdAt,
                updatedAt: updatedAt,
                failureMessage: failureMessage,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$JobsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $JobsTable,
      JobRow,
      $$JobsTableFilterComposer,
      $$JobsTableOrderingComposer,
      $$JobsTableAnnotationComposer,
      $$JobsTableCreateCompanionBuilder,
      $$JobsTableUpdateCompanionBuilder,
      (JobRow, BaseReferences<_$AppDatabase, $JobsTable, JobRow>),
      JobRow,
      PrefetchHooks Function()
    >;

class $AppDatabaseManager {
  final _$AppDatabase _db;
  $AppDatabaseManager(this._db);
  $$JobsTableTableManager get jobs => $$JobsTableTableManager(_db, _db.jobs);
}
