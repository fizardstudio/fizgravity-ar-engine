/**
 * @file ar_bridge.h
 * @brief Jembatan C++ untuk interop memori dan bindings API dengan modul Rust Fizgravity AR.
 * Mendukung tracking VIO EKF, Face Mesh, Hand tracking, Physics, 3DGS, dan kolaborasi P2P.
 */

#ifndef AR_BRIDGE_H
#define AR_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>

/**
 * @brief Jumlah konstanta mesh wajah (468 vertices standar MediaPipe/ARKit).
 */
#define FACE_MESH_VERTICES_COUNT 468

/**
 * @brief Jumlah parameter blendshapes wajah standar (52 ARKit blendshapes).
 */
#define FACE_BLENDSHAPES_COUNT 52

/**
 * @brief Jumlah sendi tangan standar (21 joints).
 */
#define HAND_JOINTS_COUNT 21

/**
 * @brief Titik 3D vertikal dalam ruang koordinat wajah lokal.
 */
typedef struct {
    float x;
    float y;
    float z;
} ArVertex3D;

/**
 * @brief Jaring wajah 3D dan koefisien blendshape untuk FFI.
 */
typedef struct {
    ArVertex3D vertices[FACE_MESH_VERTICES_COUNT];
    float blendshapes[FACE_BLENDSHAPES_COUNT];
} ArFaceMesh;

/**
 * @brief Sendi koordinat tangan 3D untuk FFI.
 */
typedef struct {
    ArVertex3D joints[HAND_JOINTS_COUNT];
    float confidence;
    int32_t is_right_hand;
} ArHandJoints;

/**
 * @brief Representasi 3D pose kamera melintasi FFI C-ABI.
 */
typedef struct {
    float position[3]; // X, Y, Z (dalam meter)
    float rotation[4]; // Kuaternion (W, X, Y, Z)
} ArPose;

/**
 * @brief Koefisien Harmonik Sferis (SH) Orde 2 untuk RGB (total 27 floats).
 */
typedef struct {
    float coefficients_r[9];
    float coefficients_g[9];
    float coefficients_b[9];
} ArSphericalHarmonics;

/**
 * @brief Data struktur representasi satu 3D Gaussian Splat.
 */
typedef struct {
    ArVertex3D position;
    ArVertex3D scale;
    float rotation[4];
    float opacity;
    float color_sh[9];
} ArGaussianSplat;

/**
 * @brief Kunci Voxel Hash spasial 3D untuk sinkronisasi desentralisasi P2P.
 */
typedef struct {
    int32_t x;
    int32_t y;
    int32_t z;
    float confidence;
} ArVoxelHashKey;

/**
 * @brief Menginisialisasi instansi baru dari Fizgravity AR Engine di sisi Rust.
 * @return Pointer mentah void* ke objek FizgravityEngine internal.
 */
void* fizgravity_engine_init();

/**
 * @brief Memperbarui pelacakan frame AR.
 */
int32_t fizgravity_engine_update_frame(
    void* engine_ptr,
    float timestamp,
    const void* camera_data,
    const void* imu_data,
    ArPose* out_pose,
    ArSphericalHarmonics* out_lighting
);

/**
 * @brief Mengekstrak jaring wajah 3D terupdate.
 */
int32_t fizgravity_engine_get_face_mesh(void* engine_ptr, ArFaceMesh* out_mesh);

/**
 * @brief Mengekstrak sendi tangan 3D terupdate.
 */
int32_t fizgravity_engine_get_hand_joints(void* engine_ptr, int32_t is_right, ArHandJoints* out_joints);

/**
 * @brief Menambahkan plane collider ke simulator fisika.
 */
int32_t fizgravity_engine_add_physics_plane(void* engine_ptr, int32_t id, float x, float y, float z);

/**
 * @brief Menjalankan langkah simulasi fisika.
 */
int32_t fizgravity_engine_step_physics(void* engine_ptr, float delta_time);

/**
 * @brief Mengekstrak data 3D Gaussian Splats untuk render pipeline.
 */
int32_t fizgravity_engine_get_gaussian_splats(void* engine_ptr, ArGaussianSplat* out_splats, int32_t max_count);

/**
 * @brief Mengepaskan (fitting) awan titik mentah ke dalam representasi 3D Gaussians.
 */
int32_t fizgravity_engine_fit_point_cloud_to_gaussians(void* engine_ptr, const ArVertex3D* points, int32_t count);

/**
 * @brief Memulai pemindaian perangkat P2P terdekat.
 */
int32_t fizgravity_engine_p2p_start_discovery(void* engine_ptr);

/**
 * @brief Sinkronisasi voxel delta keys dengan perangkat terdekat.
 */
int32_t fizgravity_engine_p2p_sync_voxels(void* engine_ptr, const ArVoxelHashKey* keys, int32_t count);

/**
 * @brief Membebaskan memori objek FizgravityEngine.
 * @param engine_ptr Pointer ke FizgravityEngine yang akan dibebaskan.
 */
void fizgravity_engine_release(void* engine_ptr);

#ifdef __cplusplus
}
#endif

// Abstraksi berorientasi objek C++ tingkat tinggi untuk kemudahan integrasi
#ifdef __cplusplus
namespace fizgravity {

class Engine {
public:
    Engine() {
        m_engine_ptr = fizgravity_engine_init();
    }

    ~Engine() {
        if (m_engine_ptr != nullptr) {
            fizgravity_engine_release(m_engine_ptr);
        }
    }

    bool update(float timestamp, const void* camera_data, const void* imu_data, 
                ArPose& out_pose, ArSphericalHarmonics& out_lighting) {
        if (m_engine_ptr == nullptr) return false;
        
        int32_t result = fizgravity_engine_update_frame(
            m_engine_ptr, 
            timestamp, 
            camera_data, 
            imu_data, 
            &out_pose, 
            &out_lighting
        );
        
        return result == 0;
    }

    bool getFaceMesh(ArFaceMesh& out_mesh) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_face_mesh(m_engine_ptr, &out_mesh) == 0;
    }

    bool getHandJoints(bool is_right, ArHandJoints& out_joints) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_hand_joints(m_engine_ptr, is_right ? 1 : 0, &out_joints) == 0;
    }

    bool addPhysicsPlane(int32_t id, float x, float y, float z) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_add_physics_plane(m_engine_ptr, id, x, y, z) == 0;
    }

    bool stepPhysics(float delta_time) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_step_physics(m_engine_ptr, delta_time) == 0;
    }

    int32_t getGaussianSplats(ArGaussianSplat* out_splats, int32_t max_count) {
        if (m_engine_ptr == nullptr) return 0;
        return fizgravity_engine_get_gaussian_splats(m_engine_ptr, out_splats, max_count);
    }

    bool fitPointCloudToGaussians(const ArVertex3D* points, int32_t count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_fit_point_cloud_to_gaussians(m_engine_ptr, points, count) == 0;
    }

    bool p2pStartDiscovery() {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_p2p_start_discovery(m_engine_ptr) == 0;
    }

    bool p2pSyncVoxels(const ArVoxelHashKey* keys, int32_t count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_p2p_sync_voxels(m_engine_ptr, keys, count) == 0;
    }

    // Hindari penyalinan objek mesin secara tidak sengaja untuk mencegah double free
    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

private:
    void* m_engine_ptr = nullptr;
};

} // namespace fizgravity
#endif

#endif // AR_BRIDGE_H
